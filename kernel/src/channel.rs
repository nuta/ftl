use alloc::collections::vec_deque::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::min;
use core::mem;

use ftl_types::channel::MessageInfo;
use ftl_types::channel::Peek;
use ftl_types::channel::RecvToken;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::Notifier;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

enum EntryState {
    Pending {
        info: MessageInfo,
        arg1: usize,
        arg2: usize,
    },
    Notified(RecvToken),
}

struct Entry {
    state: EntryState,
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

struct TokenAllocator {
    bitmap: u64,
}

impl TokenAllocator {
    const fn new() -> Self {
        Self { bitmap: 0 }
    }

    fn is_full(&self) -> bool {
        self.bitmap == u64::MAX
    }

    fn alloc(&mut self) -> Option<RecvToken> {
        let i = self.bitmap.trailing_ones();
        if i == 64 {
            return None;
        }
        self.bitmap |= 1 << i;
        Some(RecvToken::new(i as u16))
    }

    fn free(&mut self, token: RecvToken) {
        let index = token.as_u16() as usize;
        debug_assert!(self.bitmap & (1 << index) != 0);
        self.bitmap &= !(1 << index);
    }
}

struct Mutable {
    /// The channel state.
    state: State,
    /// Received messages.
    queue: VecDeque<Entry>,
    /// The sink waker.
    notifier: Option<Notifier>,
    /// [`RecvToken`] allocator.
    token_allocator: TokenAllocator,
}

pub struct Channel {
    mutable: SpinLock<Mutable>,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Self>, SharedRef<Self>), ErrorCode> {
        let ch0 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::Initializing,
                queue: VecDeque::new(),
                notifier: None,
                token_allocator: TokenAllocator::new(),
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::Connected(ch0.clone()),
                queue: VecDeque::new(),
                notifier: None,
                token_allocator: TokenAllocator::new(),
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

        peer_mutable.queue.push_back(Entry {
            state: EntryState::Pending { info, arg1, arg2 },
            body,
            handle,
        });

        if let Some(ref notifier) = peer_mutable.notifier {
            notifier.notify();
        }

        Ok(())
    }

    fn recv(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        handle_table: &mut HandleTable,
        token: RecvToken,
        body_slice: UserSlice,
    ) -> Result<HandleId, ErrorCode> {
        let mut mutable = self.mutable.lock();
        for (i, entry) in mutable.queue.iter_mut().enumerate() {
            if matches!(entry.state, EntryState::Notified(t) if t == token) {
                let slot = if entry.handle.is_some() {
                    Some(handle_table.reserve()?)
                } else {
                    None
                };

                // Copy the body first since it may fail.
                if let Some(ref body) = entry.body {
                    let copy_len = min(body_slice.len(), body.len());
                    let slice = body_slice.subslice(0, copy_len)?;
                    isolation.write_bytes(&slice, &body[..copy_len])?;
                }

                // Point of no return: All operations after this point must
                // succeed to guarantee that "if sys_channel_recv returns an
                // error, the message is kept in the queue".
                let entry = mutable.queue.remove(i).unwrap();

                let handle_id = if let Some(handle) = entry.handle {
                    slot.unwrap().insert(handle)
                } else {
                    HandleId::from_raw(0)
                };

                let was_full = mutable.token_allocator.is_full();
                mutable.token_allocator.free(token);
                if let Some(ref notifier) = mutable.notifier {
                    let drained =
                        matches!(mutable.state, State::Draining) && mutable.queue.is_empty();
                    if was_full || drained {
                        notifier.notify();
                    }
                }

                return Ok(handle_id);
            }
        }

        Err(ErrorCode::NotFound)
    }

    fn discard(&self, token: RecvToken) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        for (i, entry) in mutable.queue.iter_mut().enumerate() {
            if matches!(entry.state, EntryState::Notified(t) if t == token) {
                let entry = mutable.queue.remove(i).unwrap();

                if let Some(handle) = entry.handle {
                    handle.bypass_check().close();
                }

                let was_full = mutable.token_allocator.is_full();
                mutable.token_allocator.free(token);
                if let Some(ref notifier) = mutable.notifier {
                    let drained =
                        matches!(mutable.state, State::Draining) && mutable.queue.is_empty();
                    if was_full || drained {
                        notifier.notify();
                    }
                }

                return Ok(());
            }
        }

        Err(ErrorCode::NotFound)
    }
}

impl Handleable for Channel {
    fn set_notifier(&self, notifier: Notifier) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.notifier.is_some() {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable.notifier = Some(notifier);
        Ok(())
    }

    fn remove_notifier(&self) {
        let mut mutable = self.mutable.lock();
        debug_assert!(mutable.notifier.is_some());
        mutable.notifier = None;
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
        if let Some(ref notifier) = peer_mutable.notifier {
            notifier.notify();
        }
    }

    fn poll(
        &self,
        handle_id: HandleId,
        _handle_table: &mut HandleTable,
        isolation: &SharedRef<dyn Isolation>,
        buf: &UserSlice,
    ) -> Result<bool, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Mutable {
            state,
            queue,
            token_allocator: token_bitmap,
            ..
        } = &mut *mutable;

        for entry in queue.iter_mut() {
            if let EntryState::Pending { info, arg1, arg2 } = &entry.state {
                let token = match token_bitmap.alloc() {
                    Some(token) => token,
                    None => {
                        // We have too many inflight receives. Do not return an event for now.
                        return Ok(false);
                    }
                };

                let peek = Peek {
                    info: *info,
                    token,
                    reserved: 0,
                    arg1: *arg1,
                    arg2: *arg2,
                };

                // TODO: What if isolation write fails?
                entry.state = EntryState::Notified(token);

                let header = EventHeader {
                    ty: EventType::MESSAGE,
                    id: handle_id,
                    reserved: 0,
                };
                crate::isolation::write(isolation, &buf, 0, header)?;
                crate::isolation::write(isolation, &buf, size_of::<EventHeader>(), peek)?;

                return Ok(true);
            }
        }

        match state {
            State::Initializing => unreachable!(),
            State::Connected(_) => { /* do nothing */ }
            State::Draining => {
                if queue.is_empty() {
                    *state = State::PeerClosed;
                    crate::isolation::write(
                        isolation,
                        &buf,
                        0,
                        EventHeader {
                            ty: EventType::PEER_CLOSED,
                            id: handle_id,
                            reserved: 0,
                        },
                    )?;
                    return Ok(true);
                }
            }
            State::PeerClosed => { /* do nothing */ }
        }

        Ok(false)
    }
}

pub fn sys_channel_create(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ids = UserSlice::new(UserPtr::new(a0), size_of::<[HandleId; 2]>())?;

    let process = current.process();
    let isolation = process.isolation();
    let mut handle_table = process.handle_table().lock();
    let reserved = handle_table.reserve()?;

    let (ch0, ch1) = Channel::new()?;
    let handle0 = Handle::new(ch0, HandleRight::ALL);
    let handle1 = Handle::new(ch1, HandleRight::ALL);

    let (id0, id1) = reserved.insert2(handle0, handle1);
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
    a3: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ch_id = HandleId::from_raw(a0);
    let token = RecvToken::new(a1 as u16);
    let body_ptr = UserPtr::new(a2);
    let slice = UserSlice::new(body_ptr, a3)?;

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(ch_id)?
        .authorize(HandleRight::READ)?;

    let handle_id = ch.recv(process.isolation(), &mut handle_table, token, slice)?;
    Ok(SyscallResult::Return(handle_id.as_usize()))
}

pub fn sys_channel_discard(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let ch_id = HandleId::from_raw(a0);
    let token = RecvToken::new(a1 as u16);

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(ch_id)?
        .authorize(HandleRight::READ)?;

    ch.discard(token)?;
    Ok(SyscallResult::Return(0))
}
