use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use core::cmp::min;
use core::mem::MaybeUninit;

use ftl_arrayvec::ArrayVec;
use ftl_types::channel::CallId;
use ftl_types::channel::INLINE_LEN_MAX;
use ftl_types::channel::MessageBody;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::NUM_HANDLES_MAX;
use ftl_types::channel::NUM_OOLS_MAX;
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
    Call {
        call_id: CallId,
        info: MessageInfo,
        handles: ArrayVec<AnyHandle, NUM_HANDLES_MAX>,
        inline: [u8; INLINE_LEN_MAX],
    },
    Reply {
        cookie: usize,
        info: MessageInfo,
        handles: ArrayVec<AnyHandle, NUM_HANDLES_MAX>,
        inline: [u8; INLINE_LEN_MAX],
    },
}

struct Call {
    cookie: usize,
    ools: ArrayVec<Ool, NUM_OOLS_MAX>,
}

struct Mutable {
    peer: Option<SharedRef<Channel>>,
    queue: VecDeque<Message>,
    emitter: Option<EventEmitter>,
    calls: BTreeMap<u32 /* call id */, Call>,
    next_call_id: u32,
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
                calls: BTreeMap::new(),
                next_call_id: 1,
                peer_closed_notified: false,
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: Some(ch0.clone()),
                queue: VecDeque::new(),
                emitter: None,
                calls: BTreeMap::new(),
                next_call_id: 1,
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
        call_id: CallId,
    ) -> Result<(), ErrorCode> {
        if info.num_handles() > NUM_HANDLES_MAX {
            return Err(ErrorCode::InvalidMessage);
        }

        if info.num_ools() > NUM_OOLS_MAX {
            return Err(ErrorCode::InvalidMessage);
        }

        if info.inline_len() > INLINE_LEN_MAX {
            return Err(ErrorCode::InvalidMessage);
        }

        let mut mutable = self.mutable.lock();
        let peer = mutable.peer.as_ref().ok_or(ErrorCode::PeerClosed)?.clone();

        let body: MessageBody = crate::isolation::read(isolation, body_slice, 0)?;

        let mut handles = ArrayVec::new();
        for i in 0..info.num_handles() {
            // TODO: Check if all handles can be transferred. That is, make this
            //       operation atomic.
            let handle = handle_table.remove(body.handles[i])?;
            if handles.try_push(handle).is_err() {
                // We've checked the # of handles in the MessageInfo above.
                unreachable!();
            }
        }
        let call = if info.is_call() {
            None
        } else {
            Some(
                mutable
                    .calls
                    .remove(&call_id.as_u32())
                    .ok_or(ErrorCode::InvalidMessage)?,
            )
        };

        // Drop the lock before acquiring the peer's lock. Otherwise, we may
        // deadlock since the lock order is not guaranteed.
        drop(mutable);

        let mut peer_mutable = peer.mutable.lock();

        let message = if info.is_call() {
            let call_id: CallId = CallId::new(peer_mutable.next_call_id);
            assert!(!peer_mutable.calls.contains_key(&call_id.as_u32())); // FIXME: Retry with a different ID
            peer_mutable.next_call_id += 1; // FIXME: wrapping around

            let mut ools = ArrayVec::new();
            for i in 0..info.num_ools() {
                let ptr = UserPtr::new(body.ools[i].addr);
                // FIXME: We should not fail here.
                let slice = UserSlice::new(ptr, body.ools[i].len)?;
                let ool = Ool {
                    isolation: isolation.clone(),
                    slice,
                };

                if ools.try_push(ool).is_err() {
                    // We've checked the # of ools in the MessageInfo above.
                    unreachable!();
                }
            }

            peer_mutable
                .calls
                .insert(call_id.as_u32(), Call { cookie, ools });

            Message::Call {
                call_id,
                info,
                handles,
                inline: body.inline,
            }
        } else {
            Message::Reply {
                cookie: call.unwrap().cookie, // TODO: refactor
                info,
                handles,
                inline: body.inline,
            }
        };

        peer_mutable.queue.push_back(message);
        if let Some(ref emitter) = peer_mutable.emitter {
            emitter.notify();
        }
        println!(
            "enqueued a message: kind={}, {} OOLs, {} handles, {} bytes",
            info.kind(),
            info.num_ools(),
            info.num_handles(),
            info.inline_len()
        );

        Ok(())
    }

    pub fn read_ool(
        &self,
        dst_isolation: &SharedRef<dyn Isolation>,
        call_id: CallId,
        index: usize,
        offset: usize,
        dst_slice: &UserSlice,
    ) -> Result<usize, ErrorCode> {
        let mutable = self.mutable.lock();
        let call = mutable
            .calls
            .get(&call_id.as_u32())
            .ok_or(ErrorCode::InvalidArgument)?;

        let ool = call.ools.get(index).ok_or(ErrorCode::InvalidArgument)?;
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
        call_id: CallId,
        index: usize,
        offset: usize,
        src_slice: &UserSlice,
    ) -> Result<usize, ErrorCode> {
        let mutable = self.mutable.lock();
        let call = mutable
            .calls
            .get(&call_id.as_u32())
            .ok_or(ErrorCode::InvalidArgument)?;

        let ool = call.ools.get(index).ok_or(ErrorCode::InvalidArgument)?;
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

        // FIXME: Ugly unsafe code. This leaks kernel memory.
        let event = MaybeUninit::<MessageEvent>::uninit();
        let mut event = unsafe { event.assume_init() };

        let (info, mut handles, inline) = match message {
            Message::Call {
                call_id,
                info,
                handles,
                inline,
            } => {
                event.call_id = call_id;
                (info, handles, inline)
            }
            Message::Reply {
                cookie,
                info,
                handles,
                inline,
            } => {
                event.cookie = cookie;
                (info, handles, inline)
            }
        };

        let inline_len = info.inline_len();
        event.info = info;
        event.body.inline[..inline_len].copy_from_slice(&inline[..inline_len]);

        // Move handles to our process.
        debug_assert_eq!(handles.len(), info.num_handles());
        for i in 0..info.num_handles() {
            let handle = handles.pop().unwrap();
            let id = handle_table.insert(handle)?; // TODO: What if this fails?
            event.body.handles[i] = id;
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
    let body = UserSlice::new(UserPtr::new(a2), size_of::<MessageBody>())?;
    let cookie = a3;
    let call_id = CallId::new(a4 as u32);

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
        call_id,
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
    let call_id = CallId::new((a1 >> 4) as u32);
    let index = a1 & 0b1111;
    let offset = a2;
    let buf = UserSlice::new(UserPtr::new(a3), a4)?;

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::READ)?;

    let read_len = ch.read_ool(process.isolation(), call_id, index, offset, &buf)?;
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
    let call_id = CallId::new((a1 >> 4) as u32);
    let index = a1 & 0b1111;
    let offset = a2;
    let buf = UserSlice::new(UserPtr::new(a3), a4)?;

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::WRITE)?;

    let written_len = ch.write_ool(process.isolation(), call_id, index, offset, &buf)?;
    Ok(SyscallResult::Return(written_len))
}
