use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::mem::MaybeUninit;

use ftl_types::error::FtlError;
use ftl_types::message::MessageInfo;

use crate::handle::Handleable;
use crate::poll::PollPoint;
use crate::poll::PollResult;
use crate::ref_counted::SharedRef;
use crate::ref_counted::UniqueRef;
use crate::spinlock::SpinLock;

pub struct MessageEntry {
    msginfo: MessageInfo,
    data: Vec<u8>,
}

pub struct Channel {
    peer: SharedRef<Channel>,
    queue: SpinLock<VecDeque<MessageEntry>>,
    event_point: PollPoint,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Channel>, SharedRef<Channel>), FtlError> {
        let ch0: UniqueRef<MaybeUninit<Channel>> = UniqueRef::new(MaybeUninit::uninit());
        let ch1: UniqueRef<MaybeUninit<Channel>> = UniqueRef::new(MaybeUninit::uninit());
        let ch0_sref: SharedRef<Channel> = UniqueRef::as_shared_ref(&mut ch0);
        let ch1_sref: SharedRef<Channel> = UniqueRef::as_shared_ref(&mut ch1);

        ch0.write(Channel { peer: , queue: (), event_point: () })

        todo!()
    }

    // TODO: user pointers
    pub fn send(&self, msginfo: MessageInfo, data: &[u8]) -> Result<(), FtlError> {
        let data = data.to_vec();
        let entry = MessageEntry { msginfo, data };

        let mut peer_queue = self.peer.queue.lock();
        peer_queue.push_back(entry);
        self.peer.event_point.wake();

        Ok(())
    }

    pub fn recv(&self) -> Result<MessageInfo, FtlError> {
        self.event_point.poll_loop(&self.queue, |queue| {
            if let Some(entry) = queue.pop_front() {
                return PollResult::Ready(Ok(entry.msginfo));
            }

            PollResult::Sleep
        })
    }
}

impl Handleable for Channel {}
