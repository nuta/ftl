use ftl_types::error::FtlError;

use crate::handle::Handle;

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

// FIXME: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

pub struct Channel {
    raw: Handle,
}

impl Channel {
    pub fn new() -> Result<(Channel, Channel), FtlError> {
        todo!()
    }

    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        todo!()
    }

    pub fn receive(&mut self) -> Result<Message, FtlError> {
        todo!()
    }

    pub fn call(&mut self, message: Message) -> Result<Message, CallError> {
        todo!()
    }
}
