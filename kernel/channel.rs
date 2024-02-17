use alloc::collections::VecDeque;
use alloc::sync::Arc;
use ftl_types::error::FtlError;
use ftl_types::event_poll::Event;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageOrSignal;
use ftl_types::signal::Signal;
use ftl_types::signal::SignalSet;
use ftl_types::Message;

use crate::arch::cpuvar_ref;
use crate::arch::yield_cpu;
use crate::arch::{self};
use crate::event_poll::EventPoll;
use crate::fiber::Fiber;
use crate::lock::Mutex;
use crate::scheduler::GLOBAL_SCHEDULER;

#[derive(Debug)]
pub struct SendError {
    pub error: FtlError,
    pub message: Message,
}

#[derive(Debug)]
pub enum CallError {
    SendError(SendError),
    ReceiveError(FtlError),
}

struct RawChannel {
    peer: Option<Arc<Mutex<RawChannel>>>,
    rx_queue: VecDeque<Message>,
    capacity: usize,
    receiver: Option<Arc<Mutex<Fiber>>>,
    event_poll: Option<(HandleId, EventPoll)>,
    signals: SignalSet,
}

impl RawChannel {
    pub fn new() -> (Arc<Mutex<RawChannel>>, Arc<Mutex<RawChannel>>) {
        let raw_a = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            capacity: 16, // TODO:
            receiver: None,
            event_poll: None,
            signals: SignalSet::zeroed(),
        };

        let raw_b = RawChannel {
            peer: None,
            rx_queue: VecDeque::new(),
            capacity: 16, // TODO:
            receiver: None,
            event_poll: None,
            signals: SignalSet::zeroed(),
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
                error: FtlError::ClosedByPeer,
                message,
            });
        };

        let mut peer = peer_lock.lock();
        if peer.rx_queue.len() >= peer.capacity {
            return Err(SendError {
                error: FtlError::Full,
                message,
            });
        }

        peer.rx_queue.push_back(message);

        if let Some((handle_id, event_poll)) = peer.event_poll.as_ref() {
            event_poll.notify(*handle_id, Event::READABLE);
        }

        if let Some(receiver) = peer.receiver.as_ref() {
            GLOBAL_SCHEDULER.lock().resume(receiver.clone());
        }

        Ok(())
    }

    pub fn notify(&mut self, signal: Signal) -> Result<(), FtlError> {
        let Some(peer_lock) = self.peer.as_ref() else {
            return Err(FtlError::ClosedByPeer);
        };

        let mut peer = peer_lock.lock();
        peer.signals.add(signal);

        if let Some((handle_id, event_poll)) = peer.event_poll.as_ref() {
            event_poll.notify(*handle_id, Event::READABLE);
        }

        if let Some(receiver) = peer.receiver.take() {
            GLOBAL_SCHEDULER.lock().resume(receiver);
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<MessageOrSignal>, FtlError> {
        loop {
            if !self.signals.is_empty() {
                let signals = self.signals.clear();
                return Ok(Some(MessageOrSignal::Signal(signals)));
            }

            if let Some(message) = self.rx_queue.pop_front() {
                return Ok(Some(MessageOrSignal::Message(message)));
            }

            let current = cpuvar_ref().current.clone();
            GLOBAL_SCHEDULER.lock().block(&current);
            self.receiver = Some(current);
            arch::yield_cpu();
            self.receiver = None;
        }
    }

    pub fn poll_in(&mut self, handle_id: HandleId, event_poll: &EventPoll) -> Result<(), FtlError> {
        if self.event_poll.is_some() {
            return Err(FtlError::AlreadyExists);
        }

        self.event_poll = Some((handle_id, event_poll.clone()));
        Ok(())
    }
}

#[derive(Clone)]
pub struct Channel {
    raw: Arc<Mutex<RawChannel>>,
}

impl Channel {
    pub fn new() -> Result<(Channel, Channel), FtlError> {
        let (raw1, raw2) = RawChannel::new();
        let ch1 = Channel { raw: raw1 };
        let ch2 = Channel { raw: raw2 };
        Ok((ch1, ch2))
    }

    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        self.raw.lock().send(message)
    }

    pub fn notify(&mut self, signal: Signal) -> Result<(), FtlError> {
        self.raw.lock().notify(signal)
    }

    pub fn receive(&mut self) -> Result<MessageOrSignal, FtlError> {
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
                    println!(">>> receive: yielding CPU...");
                    yield_cpu();
                }
            }
        }
    }

    pub fn call(&mut self, message: Message) -> Result<MessageOrSignal, CallError> {
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

    pub fn poll_in(&mut self, handle_id: HandleId, event_poll: &EventPoll) -> Result<(), FtlError> {
        self.raw.lock().poll_in(handle_id, event_poll)
    }
}
