use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use core::cmp::min;
use core::mem::MaybeUninit;

use ftl_types::channel::MessageInfo;
use ftl_types::channel::RawMessage;
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

struct Ool {
    isolation: SharedRef<dyn Isolation>,
    slice: UserSlice,
}

enum Message {
    Request {
        id: RequestId,
        info: MessageInfo,
        handle: Option<AnyHandle>,
        ool_len: usize,
        inline: usize,
    },
    Reply {
        cookie: usize,
        info: MessageInfo,
        handle: Option<AnyHandle>,
        inline: usize,
    },
}

struct Call {
    cookie: usize,
    ool: Option<Ool>,
}

struct Mutable {
    peer: Option<SharedRef<Channel>>,
    queue: VecDeque<Message>,
    emitter: Option<EventEmitter>,
    requests: BTreeMap<u32 /* RequestId */, Call>,
    next_request_id: u32,
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
                requests: BTreeMap::new(),
                next_request_id: 1,
                peer_closed_notified: false,
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: Some(ch0.clone()),
                queue: VecDeque::new(),
                emitter: None,
                requests: BTreeMap::new(),
                next_request_id: 1,
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
        body_slice: &UserSlice,
        cookie: usize,
        request_id: RequestId,
    ) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let peer = mutable.peer.as_ref().ok_or(ErrorCode::PeerClosed)?.clone();

        let body: RawMessage = crate::isolation::read(isolation, body_slice, 0)?;

        let handle = if info.contains_handle() {
            Some(handle_table.remove(body.handle)?)
        } else {
            None
        };

        let call = if info.is_call() {
            None
        } else {
            Some(
                mutable
                    .requests
                    .remove(&request_id.as_u32())
                    .ok_or(ErrorCode::InvalidMessage)?,
            )
        };

        // Drop the lock before acquiring the peer's lock. Otherwise, we may
        // deadlock since the lock order is not guaranteed.
        drop(mutable);

        let mut peer_mutable = peer.mutable.lock();

        let message = if info.is_call() {
            let id = RequestId::new(peer_mutable.next_request_id);
            assert!(!peer_mutable.requests.contains_key(&id.as_u32())); // FIXME: Retry with a different ID
            peer_mutable.next_request_id += 1; // FIXME: wrapping around

            let ool = if info.contains_ool() {
                let ptr = UserPtr::new(body.ool_addr);
                let slice = UserSlice::new(ptr, body.ool_len)?;
                Some(Ool {
                    isolation: isolation.clone(),
                    slice,
                })
            } else {
                None
            };

            peer_mutable
                .requests
                .insert(id.as_u32(), Call { cookie, ool });

            Message::Request {
                id,
                info,
                handle,
                ool_len: body.ool_len,
                inline: body.inline,
            }
        } else {
            Message::Reply {
                cookie: call.unwrap().cookie, // TODO: refactor
                info,
                handle,
                inline: body.inline,
            }
        };

        peer_mutable.queue.push_back(message);
        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
        }

        Ok(())
    }

    pub fn read_ool(
        &self,
        dst_isolation: &SharedRef<dyn Isolation>,
        request_id: RequestId,
        index: usize,
        offset: usize,
        dst_slice: &UserSlice,
    ) -> Result<usize, ErrorCode> {
        if index != 0 {
            return Err(ErrorCode::InvalidArgument);
        }

        let mutable = self.mutable.lock();
        let call = mutable
            .requests
            .get(&request_id.as_u32())
            .ok_or(ErrorCode::InvalidArgument)?;

        let ool = call.ool.as_ref().ok_or(ErrorCode::InvalidArgument)?;
        let src_isolation = &ool.isolation;
        let src_slice = &ool.slice;

        let requested_len = min(dst_slice.len(), src_slice.len().saturating_sub(offset));
        let mut off = 0;
        while off < requested_len {
            // TODO: Do not zero the memory.
            let mut tmp = [0; 512];

            // Copy from the sender process' memory into the kernel's memory.
            let copy_len = min(requested_len - off, tmp.len());
            src_isolation.read_bytes(
                &src_slice.subslice(offset + off, copy_len)?,
                &mut tmp[..copy_len],
            )?;

            // Copy into the receiver (current) process' memory.
            dst_isolation.write_bytes(&dst_slice.subslice(off, copy_len)?, &tmp[..copy_len])?;

            off += copy_len;
        }

        Ok(off)
    }

    pub fn write_ool(
        &self,
        src_isolation: &SharedRef<dyn Isolation>,
        request_id: RequestId,
        index: usize,
        offset: usize,
        src_slice: &UserSlice,
    ) -> Result<usize, ErrorCode> {
        if index != 0 {
            return Err(ErrorCode::InvalidArgument);
        }

        let mutable = self.mutable.lock();
        let call = mutable
            .requests
            .get(&request_id.as_u32())
            .ok_or(ErrorCode::InvalidArgument)?;

        let ool = call.ool.as_ref().ok_or(ErrorCode::InvalidArgument)?;
        let dst_isolation = &ool.isolation;
        let dst_slice = &ool.slice;

        let requested_len = min(src_slice.len(), dst_slice.len().saturating_sub(offset));
        let mut off = 0;
        while off < requested_len {
            // TODO: Do not zero the memory.
            let mut tmp = [0; 512];

            // Copy from the receiver (current) process' memory into the kernel's memory.
            let copy_len = min(requested_len - off, tmp.len());
            src_isolation.read_bytes(&src_slice.subslice(off, copy_len)?, &mut tmp[..copy_len])?;

            // Copy into the sender process' memory.
            dst_isolation.write_bytes(
                &dst_slice.subslice(offset + off, copy_len)?,
                &tmp[..copy_len],
            )?;

            off += copy_len;
        }

        Ok(off)
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
        let Some(message) = mutable.queue.pop_front() else {
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
        };

        let mut event = unsafe { MaybeUninit::<MessageEvent>::zeroed().assume_init() };

        let handle = match message {
            Message::Request {
                id,
                info,
                handle,
                ool_len,
                inline,
            } => {
                event.info = info;
                event.inline = inline;
                event.request_id = id;
                event.ool_len = ool_len;
                handle
            }
            Message::Reply {
                cookie,
                info,
                handle,
                inline,
            } => {
                event.info = info;
                event.inline = inline;
                event.cookie = cookie;
                handle
            }
        };

        if let Some(handle) = handle {
            let id = handle_table.insert(handle)?; // TODO: What if this fails?
            event.handle = id;
        }

        Ok(Some((EventType::MESSAGE, EventBody { message: event })))
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
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1 as u32);
    let body = UserSlice::new(UserPtr::new(a2), size_of::<RawMessage>())?;
    let cookie = a3;
    let request_id = RequestId::new(a4 as u32);

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::WRITE)?;

    ch.send(
        process.isolation(),
        &mut handle_table,
        info,
        &body,
        cookie,
        request_id,
    )?;

    Ok(SyscallResult::Return(0))
}

pub fn sys_channel_ool_read(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);
    let request_id = RequestId::new((a1 >> 4) as u32);
    let index = a1 & 0b1111;
    let offset = a2;
    let buf = UserSlice::new(UserPtr::new(a3), a4)?;

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::READ)?;

    let read_len = ch.read_ool(process.isolation(), request_id, index, offset, &buf)?;
    Ok(SyscallResult::Return(read_len))
}

pub fn sys_channel_ool_write(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);
    let request_id = RequestId::new((a1 >> 4) as u32);
    let index = a1 & 0b1111;
    let offset = a2;
    let buf = UserSlice::new(UserPtr::new(a3), a4)?;

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::WRITE)?;

    let written_len = ch.write_ool(process.isolation(), request_id, index, offset, &buf)?;
    Ok(SyscallResult::Return(written_len))
}
