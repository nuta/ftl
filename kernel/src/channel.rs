use alloc::collections::vec_deque::VecDeque;

use ftl_arrayvec::ArrayVec;
use ftl_types::channel::INLINE_LEN_MAX;
use ftl_types::channel::MessageBody;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::NUM_HANDLES_MAX;
use ftl_types::channel::NUM_OOLS_MAX;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::Event;
use ftl_types::sink::EventType;
use ftl_types::sink::MessageEvent;

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
use crate::thread::Thread;

struct Message {
    info: MessageInfo,
    handles: ArrayVec<AnyHandle, NUM_HANDLES_MAX>,
    ools: ArrayVec<(SharedRef<dyn Isolation>, UserSlice), NUM_OOLS_MAX>,
    inline: [u8; INLINE_LEN_MAX],
    cookie: usize,
}

struct Mutable {
    peer: Option<SharedRef<Channel>>,
    queue: VecDeque<Message>,
    emitter: Option<EventEmitter>,
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
            }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: Some(ch0.clone()),
                queue: VecDeque::new(),
                emitter: None,
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
        body_slice: UserSlice,
        cookie: usize,
    ) -> Result<(), ErrorCode> {
        if info.num_handles() > NUM_HANDLES_MAX {
            return Err(ErrorCode::InvalidMessage);
        }

        if info.num_ools() > NUM_OOLS_MAX {
            return Err(ErrorCode::InvalidMessage);
        }

        let peer = {
            let mutable = self.mutable.lock();
            mutable.peer.as_ref().ok_or(ErrorCode::PeerClosed)?.clone()
        };

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

        let mut ools = ArrayVec::new();
        for i in 0..info.num_ools() {
            let ptr = UserPtr::new(body.ools[i].addr);
            let slice = UserSlice::new(ptr, body.ools[i].len)?;
            if ools.try_push((isolation.clone(), slice)).is_err() {
                // We've checked the # of ools in the MessageInfo above.
                unreachable!();
            }
        }

        let mut peer_mutable = peer.mutable.lock();
        peer_mutable.queue.push_back(Message {
            info,
            handles,
            ools,
            inline: body.inline,
            cookie,
        });

        println!(
            "enqueued a message: kind={}, {} OOLs, {} handles, {} bytes",
            info.kind(),
            info.num_ools(),
            info.num_handles(),
            info.inline_len()
        );

        Ok(())
    }
}

impl Handleable for Channel {
    fn set_event_emitter(&self, emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable.emitter = emitter;
        Ok(())
    }

    fn read_event(&self) -> Result<Option<(EventType, Event)>, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(message) = mutable.queue.pop_front() else {
            return Ok(None);
        };

        let event = MessageEvent {
            info: message.info,
            cookie: message.cookie,
            body: message.body,
        };

        Ok(Some((EventType::MESSAGE, Event { message: event })))
    }
}

pub fn sys_channel_create(current: &SharedRef<Thread>, a0: usize) -> Result<usize, ErrorCode> {
    let ids = UserSlice::new(UserPtr::new(a0), size_of::<[HandleId; 2]>())?;

    let (ch0, ch1) = Channel::new()?;
    let handle0 = Handle::new(ch0, HandleRight::ALL);
    let handle1 = Handle::new(ch1, HandleRight::ALL);

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let id0 = handle_table.insert(handle0)?;
    let id1 = handle_table.insert(handle1)?;

    let isolation = process.isolation();
    crate::isolation::write(isolation, ids, 0, [id0, id1])?;

    Ok(0)
}

pub fn sys_channel_send(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);
    let info = MessageInfo::from_raw(a1 as u32);
    let body = UserSlice::new(UserPtr::new(a2), size_of::<MessageBody>())?;
    let cookie = a3;

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let ch = handle_table
        .get::<Channel>(handle_id)?
        .authorize(HandleRight::WRITE)?;

    ch.send(process.isolation(), &mut handle_table, info, body, cookie)?;
    Ok(0)
}
