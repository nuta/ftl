use alloc::{collections::VecDeque, sync::Arc};

use crate::arch::yield_cpu;
use crate::result::Error;
use crate::sync::mutex::Mutex;
use crate::task::fiber::RawFiber;
use crate::task::scheduler::GLOBAL_SCHEDULER;

#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

#[derive(Debug)]
pub struct SendError {
    pub error: Error,
    pub message: Message,
}

#[derive(Debug)]
pub enum CallError {
    SendError(SendError),
    ReceiveError(Error),
}

pub(crate) struct RawChannel {
    peer: Option<Arc<Mutex<RawChannel>>>,
    rx_queue: VecDeque<Message>,
    capacity: usize,
    receiver: Option<Arc<Mutex<RawFiber>>>,
}

impl RawChannel {
    pub fn new() -> (Arc<Mutex<RawChannel>>, Arc<Mutex<RawChannel>>) {
        let raw_a = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            capacity: 16, // TODO:
            receiver: None,
        };

        let raw_b = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            capacity: 16, // TODO:
            receiver: None,
        };

        let a = Arc::new(Mutex::new(raw_a));
        let b = Arc::new(Mutex::new(raw_b));
        a.lock().peer = Some(b.clone());
        b.lock().peer = Some(a.clone());
        (a, b)
    }

    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        let Some(peer_lock) = self.peer.as_ref() else {
            return Err(SendError {
                error: Error::ClosedByPeer,
                message,
            });
        };

        let mut peer = peer_lock.lock();
        if peer.rx_queue.len() >= peer.capacity {
            return Err(SendError {
                error: Error::Full,
                message,
            });
        }

        peer.rx_queue.push_back(message);
        if let Some(receiver) = peer.receiver.as_ref() {
            receiver.lock().resume_if_blocked();
        }

        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<Message>, Error> {
        if let Some(message) = self.rx_queue.pop_front() {
            return Ok(Some(message));
        } else {
            let current = GLOBAL_SCHEDULER.lock().current();
            current.lock().block();
            self.receiver = Some(current);
        }

        Ok(None)
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
        self.raw.lock().send(message)
    }

    pub fn receive(&mut self) -> Result<Message, Error> {
        loop {
            let result = {
                let mut raw = self.raw.lock();
                raw.receive()?
            };

            match result {
                Some(message) => {
                    return Ok(message);
                }
                None => {
                    yield_cpu();
                }
            }
        }
    }

    pub fn call(&mut self, message: Message) -> Result<Message, CallError> {
        let mut raw = self.raw.lock();
        raw.send(message).map_err(CallError::SendError)?;

        loop {
            if let Some(message) = raw.receive().map_err(CallError::ReceiveError)? {
                return Ok(message);
            }

            drop(raw);
            todo!("wait for response");
        }
    }
}
