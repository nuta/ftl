use alloc::collections::VecDeque;
use alloc::vec::Vec;

use ftl_types::error::FtlError;
use ftl_types::message::MessageInfo;
use ftl_types::message::MESSAGE_DATA_MAX_LEN;

use crate::poll::PollPoint;
use crate::poll::PollResult;
use crate::ref_counted::SharedRef;
use crate::spinlock::SpinLock;

struct MessageEntry {
    msginfo: MessageInfo,
    data: Vec<u8>,
}

struct Mutable {
    peer: Option<SharedRef<Channel>>,
    queue: VecDeque<MessageEntry>,
}

pub struct Channel {
    mutable: SpinLock<Mutable>,
    event_point: PollPoint,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Channel>, SharedRef<Channel>), FtlError> {
        let ch0 = SharedRef::new(Channel {
            event_point: PollPoint::new(),
            mutable: SpinLock::new(Mutable {
                peer: None,
                queue: VecDeque::new(),
            }),
        });
        let ch1 = SharedRef::new(Channel {
            event_point: PollPoint::new(),
            mutable: SpinLock::new(Mutable {
                peer: None,
                queue: VecDeque::new(),
            }),
        });

        // TODO: Can we avoid this mutate-after-construct?
        ch0.mutable.lock().peer = Some(ch1.clone());
        ch1.mutable.lock().peer = Some(ch0.clone());

        Ok((ch0, ch1))
    }

    pub fn send(
        &self,
        msginfo: MessageInfo,
        buf: &[u8; MESSAGE_DATA_MAX_LEN],
    ) -> Result<(), FtlError> {
        let entry = MessageEntry {
            msginfo,
            data: buf.to_vec(),
        };

        let mutable = self.mutable.lock();
        let peer = mutable.peer.as_ref().ok_or(FtlError::NoPeer)?;
        let mut peer_mutable = peer.mutable.lock();
        peer_mutable.queue.push_back(entry);
        peer.event_point.wake();

        Ok(())
    }

    pub fn recv(&self, buf: &mut [u8; MESSAGE_DATA_MAX_LEN]) -> Result<MessageInfo, FtlError> {
        let entry = self.event_point.poll_loop(&self.mutable, |mutable| {
            if let Some(entry) = mutable.queue.pop_front() {
                return PollResult::Ready(entry);
            }

            PollResult::Sleep
        });

        buf[0..entry.msginfo.data_len()].copy_from_slice(&entry.data);
        Ok(entry.msginfo)
    }
}
