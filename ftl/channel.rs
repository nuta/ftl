use core::cell::UnsafeCell;

use alloc::collections::VecDeque;

pub struct Message {}

/// A single-producer, single-consumer FIFO queue.
struct Ring {
    messages: VecDeque<Message>,
}

/// The error type returned by [`Channel::send`].
///
/// This allows the caller to get the message back if it actually was not sent.
pub enum SendError {
    WouldBlock(Message),
    Error(crate::Error),
}

/// A single-producer, single-consumer bidirectional message queue.
pub struct Channel {}

impl Channel {
    pub fn new() -> Channel {
        todo!()
    }

    /// Sends a message.
    ///
    /// This operation is non-blocking: if the queue is full, this method will
    /// return [`crate::Error::WouldBlock`].
    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        todo!()
    }

    /// Receives a message.
    ///
    /// This operation is non-blocking: if the queue is empty, this method will
    /// return `Ok(None)`.
    pub fn recv(&mut self) -> crate::Result<Option<Message>> {
        todo!()
    }

    /// Sends a message and waits for a reply.
    pub fn call(&mut self, message: Message) -> crate::Result<Message> {
        todo!()
    }
}
