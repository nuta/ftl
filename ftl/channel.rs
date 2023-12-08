use alloc::collections::VecDeque;

use crate::{
    arch::{channel_call, channel_close, channel_create, channel_recv, channel_send},
    Error, Handle,
};

#[derive(Debug)]
pub enum Message {
    Packet { pkt: usize },
}

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
pub struct Channel {
    handle: Handle,
}

impl Channel {
    pub fn new() -> crate::Result<(Channel, Channel)> {
        let (handle1, handle2) = channel_create()?;
        Ok((Channel { handle: handle1 }, Channel { handle: handle2 }))
    }

    /// Sends a message.
    ///
    /// This operation is non-blocking: if the queue is full, this method will
    /// return [`crate::Error::WouldBlock`].
    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        channel_send(self.handle, message)
    }

    /// Receives a message.
    ///
    /// This operation is non-blocking: if the queue is empty, this method will
    /// return `Ok(None)`.
    pub fn recv(&mut self) -> crate::Result<Option<Message>> {
        channel_recv(self.handle)
    }

    /// Sends a message and waits for a reply.
    pub fn call(&mut self, message: Message) -> crate::Result<Message> {
        channel_call(self.handle, message)
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        // We ignore the error in release build because there's nothing we can
        // do. At least warn the user in debug build.
        if let Err(err) = channel_close(self.handle) {
            debug_warn!("failed to close channel: {:?}", err);
        }
    }
}
