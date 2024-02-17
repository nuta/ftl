use ftl_kernel::channel::CallError as KernelCallError;
use ftl_kernel::channel::SendError as KernelSendError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageOrSignal;
use ftl_types::{error::FtlError, Message};
use serde::Deserializer;

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

// FIXME: hard-coded for kernel fibers
pub struct Channel {
    handle: Handle,
    raw: ftl_kernel::channel::Channel,
}

impl Channel {
    pub fn from_handle(handle: Handle) -> Channel {
        let raw =
            ftl_kernel::fiber::Fiber::get_channel_by_handle(handle.id()).expect("invalid handle");

        Channel { handle, raw }
    }

    pub fn handle_id(&self) -> HandleId {
        self.handle.id()
    }

    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        match self.raw.send(message) {
            Ok(()) => Ok(()),
            Err(KernelSendError { error, message }) => Err(SendError { error, message }),
        }
    }

    pub fn receive(&mut self) -> Result<MessageOrSignal, FtlError> {
        self.raw.receive()
    }

    pub fn call(&mut self, message: Message) -> Result<MessageOrSignal, CallError> {
        match self.raw.call(message) {
            Ok(message) => Ok(message),
            Err(KernelCallError::SendError(KernelSendError { error, message })) => {
                Err(CallError::SendError(SendError { error, message }))
            }
            Err(KernelCallError::ReceiveError(error)) => Err(CallError::ReceiveError(error)),
        }
    }
}

pub fn deserialize_from_handle_id<'de, D>(deserializer: D) -> Result<Channel, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    // Parse as a JSON value.
    let value = serde_json::Value::deserialize(deserializer)?;

    // Check if the value is an integer.
    let raw_handle_id: i64 = match value.as_i64() {
        Some(handle_id) => handle_id,
        None => {
            return Err(serde::de::Error::custom("expected handle ID"));
        }
    };

    // Try to convert it to an isize.
    let handle_id: isize = match raw_handle_id.try_into() {
        Ok(handle) => handle,
        Err(_) => {
            return Err(serde::de::Error::custom("invalid handle ID"));
        }
    };

    let handle = Handle::new(HandleId::new(handle_id));
    let ch = Channel::from_handle(handle);
    Ok(ch)
}
