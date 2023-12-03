use core::cell::UnsafeCell;

use alloc::collections::VecDeque;

pub struct Message {}

/// A single-producer, single-consumer FIFO queue.
struct Ring {
    messages: VecDeque<Message>,
}

/// A single-producer, single-consumer bidirectional message queue.
pub struct Channel {}

impl Channel {
    pub fn new() -> Channel {
        todo!()
    }

    /// Send a message.
    ///
    /// This operation is non-blocking: if the queue is full, this method will
    /// return [`Error::WouldBlock`].
    pub fn send(&self, message: Message) -> crate::Result<()> {
        todo!()
    }

    /// Receive a message.
    ///
    /// This operation is non-blocking: if the queue is full, this method will
    /// return [`Error::WouldBlock`].
    pub fn recv(&self) -> crate::Result<Message> {
        todo!()
    }
}
