use alloc::collections::vec_deque::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::mem;

use ftl_types::channel::MessageInfo;
use ftl_types::channel::Peek;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::Event;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;
use ftl_types::sink::MessageEvent;
use ftl_types::sink::PeerClosedEvent;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::EventEmitter;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

struct Message {
    info: MessageInfo,
    arg1: usize,
    arg2: usize,
    body: Option<Vec<u8>>,
    handle: Option<AnyHandle>,
}

enum State {
    /// The channel is being created.
    Initializing,
    /// The peer is still connected.
    Connected(SharedRef<Channel>),
    /// The peer is closed, but there are still pending messages to receive.
    Draining,
    /// No more messages can be received.
    PeerClosed,
}

struct Mutable {
    /// The channel state.
    state: State,
    /// Pending messages that are not yet notified to the sink.
    rx_pending: VecDeque<Message>,
    /// Messages that have been notified to the sink, but not yet received.
    rx_notified: Vec<Message>,
    /// The sink waker.
    emitter: Option<EventEmitter>,
}

pub struct Channel {
    mutable: SpinLock<Mutable>,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Self>, SharedRef<Self>), ErrorCode> {
        let ch0 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::Initializing,
                rx_pending: VecDeque::new(),
                rx_notified: Vec::new(),
                emitter: None,
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::Connected(ch0.clone()),
                rx_pending: VecDeque::new(),
                rx_notified: Vec::new(),
                emitter: None,
            }),
        })?;
        ch0.mutable.lock().state = State::Connected(ch1.clone());

        Ok((ch0, ch1))
    }

    pub fn send(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        handle_table: &mut HandleTable,
        info: MessageInfo,
        arg1: usize,
        body_slice: UserSlice,
        handle_or_arg2: usize,
    ) -> Result<(), ErrorCode> {
        let mutable = self.mutable.lock();
        let peer = match &mutable.state {
            State::Initializing => unreachable!(),
            State::Connected(peer) => peer.clone(),
            _ => {
                return Err(ErrorCode::PeerClosed);
            }
        };

        let (handle, arg2) = if info.has_handle() {
            let handle_id = HandleId::from_raw(handle_or_arg2);
            let handle = handle_table.remove(handle_id)?;
            (Some(handle), 0)
        } else {
            (None, handle_or_arg2)
        };

        // Drop the lock before acquiring the peer's lock. Otherwise, we may
        // deadlock since the lock order is not guaranteed.
        drop(mutable);

        let mut peer_mutable = peer.mutable.lock();

        let body = if info.has_body() {
            let mut body = vec![0; info.body_len()];
            isolation.read_bytes(&body_slice, &mut body)?;
            Some(body)
        } else {
            None
        };

        peer_mutable.rx_pending.push_back(Message {
            info,
            arg1,
            arg2,
            body,
            handle,
        });

        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
        }

        Ok(())
    }

    fn recv(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        handle_table: &mut HandleTable,
        info: MessageInfo,
        body_slice: UserSlice,
    ) -> Result<HandleId, ErrorCode> {
        let mut mutable: crate::spinlock::SpinLockGuard<'_, Mutable> = self.mutable.lock();
        let reserve = handle_table.reserve()?;

        // Find the message in the notified queue, and remove it from the vec.
        let Some(message) = mutable
            .rx_notified
            .iter()
            .position(|message| message.info == info)
            .map(|pos| mutable.rx_notified.remove(pos))
        else {
            return Err(ErrorCode::NotFound);
        };

        // Copy the body to the isolation.
        if let Some(body) = message.body {
            debug_assert!(info.has_body());
            isolation.write_bytes(&body_slice, &body)?;
        }

        let handle_id = if let Some(handle) = message.handle {
            debug_assert!(info.has_handle());
            reserve.insert(handle)
        } else {
            HandleId::from_raw(0)
        };

        Ok(handle_id)
    }

    fn discard(&self, info: MessageInfo) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();

        // Remove the first matching message.
        let Some(pos) = mutable.rx_notified.iter().position(|m| m.info == info) else {
            return Err(ErrorCode::NotFound);
        };

        let mut message = mutable.rx_notified.remove(pos);
        if let Some(handle) = message.handle.take() {
            handle.bypass_check().close();
        }

        Ok(())
    }
}

impl Handleable for Channel {
    fn set_event_emitter(&self, emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable.emitter = emitter;
        Ok(())
    }

    fn close(&self) {
        // Take the peer to decrement its reference count.
        let peer = {
            let mut mutable = self.mutable.lock();
            match mem::replace(&mut mutable.state, State::Draining) {
                State::Initializing => unreachable!(),
                State::Connected(peer) => peer,
                State::Draining | State::PeerClosed => {
                    // The peer already cleared our peer field. Do nothing.
                    return;
                }
            }
        };

        let mut peer_mutable = peer.mutable.lock();
        let old = mem::replace(&mut peer_mutable.state, State::Draining);
        debug_assert!(matches!(old, State::Connected(_)));
        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
        }
    }

    fn read_event(
        &self,
        handle_id: HandleId,
        _handle_table: &mut HandleTable,
    ) -> Result<Option<Event>, ErrorCode> {
        let mut mutable = self.mutable.lock();

        if let Some(message) = mutable.rx_pending.pop_front() {
            let event = Event {
                message: MessageEvent {
                    header: EventHeader {
                        ty: EventType::MESSAGE,
                        id: handle_id,
                    },
                    peek: Peek {
                        info: message.info,
                        arg1: message.arg1,
                        arg2: message.arg2,
                    },
                },
            };

            mutable.rx_notified.push(message);
            return Ok(Some(event));
        };

        match mutable.state {
            State::Initializing => unreachable!(),
            State::Connected(_) => { /* do nothing */ }
            State::Draining => {
                if mutable.rx_pending.is_empty() {
                    mutable.state = State::PeerClosed;
                    return Ok(Some(Event {
                        peer_closed: PeerClosedEvent {
                            header: EventHeader {
                                ty: EventType::PEER_CLOSED,
                                id: handle_id,
                            },
                        },
                    }));
                }
            }
            State::PeerClosed => { /* do nothing */ }
        }

        return Ok(None);
    }
}

pub fn sys_channel_create(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ids = UserSlice::new(UserPtr::new(a0), size_of::<[HandleId; 2]>())?;

    let (ch0, ch1) = Channel::new()?;
    let handle0 = Handle::new(ch0, HandleRight::ALL);
    let handle1 = Handle::new(ch1, HandleRight::ALL);

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    // TODO: Reserve2?
    let id0 = handle_table.reserve()?.insert(handle0);
    let id1 = handle_table.reserve()?.insert(handle1);

    let isolation = process.isolation();
    crate::isolation::write(isolation, &ids, 0, [id0, id1])?;

    Ok(SyscallResult::Return(0))
}

pub fn sys_channel_send(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ch_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1);
    let arg1 = a2;
    let body_ptr = UserPtr::new(a3);
    let handle_or_arg2 = a4;
    let slice = UserSlice::new(body_ptr, info.body_len())?;

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(ch_id)?
        .authorize(HandleRight::WRITE)?;

    ch.send(
        process.isolation(),
        &mut handle_table,
        info,
        arg1,
        slice,
        handle_or_arg2,
    )?;
    Ok(SyscallResult::Return(0))
}

pub fn sys_channel_recv(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ch_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1);
    let body_ptr = UserPtr::new(a2);
    let slice = UserSlice::new(body_ptr, info.body_len())?;

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(ch_id)?
        .authorize(HandleRight::READ)?;

    let handle_id = ch.recv(process.isolation(), &mut handle_table, info, slice)?;
    Ok(SyscallResult::Return(handle_id.as_usize()))
}

pub fn sys_channel_discard(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ch_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1);

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(ch_id)?
        .authorize(HandleRight::READ)?;

    ch.discard(info)?;
    Ok(SyscallResult::Return(0))
}
