use alloc::{collections::VecDeque, sync::Arc};

use crate::result::Error;
use crate::sync::mutex::Mutex;
use crate::task::fiber::RawFiber;

pub enum Message {}

pub struct SendError {
    pub error: Error,
    pub message: Message,
}

pub(crate) struct RawChannel {
    peer: Option<Arc<Mutex<RawChannel>>>,
    rx_queue: VecDeque<Message>,
    receiver: Option<Arc<Mutex<RawFiber>>>,
}

impl RawChannel {
    pub fn new() -> (Arc<Mutex<RawChannel>>, Arc<Mutex<RawChannel>>) {
        let raw_a = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            receiver: None,
        };

        let raw_b = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            receiver: None,
        };

        let a = Arc::new(Mutex::new(raw_a));
        let b = Arc::new(Mutex::new(raw_b));
        a.lock().peer = Some(b.clone());
        b.lock().peer = Some(a.clone());
        (a, b)
    }
}

pub struct Channel {
    raw: Arc<Mutex<RawChannel>>,
}

impl Channel {
    pub fn new() -> Result<(Channel, Channel), Error> {
        let (raw_a, raw_b) = RawChannel::new();
        Ok((Channel { raw: raw_a }, Channel { raw: raw_b }))
    }

    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        let ch = self.raw.lock();
        let Some(peer_lock) = ch.peer.as_ref() else {
            return Err(SendError {
                error: Error::ClosedByPeer,
                message,
            });
        };

        let mut peer = peer_lock.lock();
        peer.rx_queue.push_back(message);
        if let Some(receiver) = peer.receiver.as_ref() {
            receiver.lock().resume_if_blocked();
        }

        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<Message>, Error> {
        let mut ch = self.raw.lock();
        if let Some(message) = ch.rx_queue.pop_front() {
            return Ok(Some(message));
        }

        Err(Error::Empty)
    }
}
