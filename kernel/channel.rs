use alloc::collections::VecDeque;
use alloc::vec::Vec;

use ftl_types::error::FtlError;
use ftl_types::message::MessageInfo;

use crate::handle::Handleable;
use crate::poll::PollPoint;
use crate::poll::Readiness;
use crate::ref_counted::SharedRef;
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
        todo!()
    }

    // TODO: user pointers
    pub fn send(&self, msginfo: MessageInfo, data: &[u8]) -> Result<(), FtlError> {
        let data = data.to_vec();
        let entry = MessageEntry { msginfo, data };

        let mut peer_queue = self.peer.queue.lock();
        peer_queue.push_back(entry);

        Ok(())
    }

    pub fn recv(&self) -> Result<MessageInfo, FtlError> {
        self.event_point.may_block(&self.queue, |queue| {
            if let Some(entry) = queue.pop_front() {
                return Readiness::Ready(Ok(entry.msginfo));
            }

            Readiness::Sleep
        })
    }
}

impl Handleable for Channel {}
