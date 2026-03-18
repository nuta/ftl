#![allow(unused)]
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use core::cmp::min;
use core::mem::MaybeUninit;

use ftl_types::channel::MessageInfo;
use ftl_types::channel::RequestId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventBody;
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
    arg: usize,
    body: Option<Vec<u8>>,
    handle: Option<AnyHandle>,
}

struct Mutable {
    peer: Option<SharedRef<Channel>>,
    queue: VecDeque<Message>,
    emitter: Option<EventEmitter>,
    peer_closed_notified: bool, // TODO: State: Connected(peer_ch), PeerClosed, Draining /* notified */
}

pub struct Channel {
    mutable: SpinLock<Mutable>,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Self>, SharedRef<Self>), ErrorCode> {
        let ch0 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: None,
                queue: VecDeque::new(),
                emitter: None,
                peer_closed_notified: false,
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: Some(ch0.clone()),
                queue: VecDeque::new(),
                emitter: None,
                peer_closed_notified: false,
            }),
        })?;
        ch0.mutable.lock().peer = Some(ch1.clone());

        Ok((ch0, ch1))
    }

    pub fn send(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        handle_table: &mut HandleTable,
        info: MessageInfo,
        arg: usize,
        body_ptr: UserPtr,
        handle: HandleId,
    ) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let peer = mutable.peer.as_ref().ok_or(ErrorCode::PeerClosed)?.clone();

        let handle = if info.has_handle() {
            Some(handle_table.remove(handle)?)
        } else {
            None
        };

        // Drop the lock before acquiring the peer's lock. Otherwise, we may
        // deadlock since the lock order is not guaranteed.
        drop(mutable);

        let mut peer_mutable = peer.mutable.lock();

        let body = if info.has_body() {
            let mut body = Vec::with_capacity(info.body_len());
            let slice = UserSlice::new(body_ptr, info.body_len())?;
            isolation.read_bytes(&slice, &mut body)?;
            Some(body)
        } else {
            None
        };

        peer_mutable.queue.push_back(Message {
            info,
            arg,
            body,
            handle,
        });

        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
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
            mutable.peer.take()
        };

        let Some(peer) = peer else {
            // The peer already cleared our peer field. Do nothing.
            return;
        };

        let mut peer_mutable = peer.mutable.lock();
        peer_mutable.peer = None;
        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
        }
    }

    fn read_event(
        &self,
        handle_table: &mut HandleTable,
    ) -> Result<Option<(EventType, EventBody)>, ErrorCode> {
        let mut mutable = self.mutable.lock();

        // FIXME: This always returns the first message in the queue.
        if let Some(message) = mutable.queue.front() {
            let mut event = unsafe { MaybeUninit::<MessageEvent>::zeroed().assume_init() };
            event.info = message.info;
            return Ok(Some((EventType::MESSAGE, EventBody { message: event })));
        };

        if mutable.peer.is_none() && !mutable.peer_closed_notified {
            mutable.peer_closed_notified = true;
            return Ok(Some((
                EventType::PEER_CLOSED,
                EventBody {
                    peer_closed: PeerClosedEvent {},
                },
            )));
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
    let id0 = handle_table.insert(handle0)?;
    let id1 = handle_table.insert(handle1)?;

    let isolation = process.isolation();
    crate::isolation::write(isolation, &ids, 0, [id0, id1])?;

    Ok(SyscallResult::Return(0))
}

pub fn sys_channel_send(
    current: &SharedRef<Thread>,
    a0: usize, // handle_id
    a1: usize, // info_and_body_len
    a2: usize, // rid_or_cookie
    a3: usize, // inline
    a4: usize, // body_or_handle
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1 as u32);
    let body_len = a1 >> 8;
    let rid_or_cookie = a2;
    let inline = a3;
    let body_or_handle = a4;

    todo!()
    // let process = current.process();
    // let mut handle_table = process.handle_table().lock();
    // let ch = handle_table
    //     .get::<Channel>(handle_id)?
    //     .authorize(HandleRight::WRITE)?;

    // ch.send(
    //     process.isolation(),
    //     &mut handle_table,
    //     info,
    //     rid_or_cookie,
    //     inline,
    //     body_or_handle,
    //     body_len,
    // )?;

    // Ok(SyscallResult::Return(0))
}
