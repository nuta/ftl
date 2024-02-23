use ftl_kernel::channel::CallError as KernelCallError;
use ftl_kernel::channel::SendError as KernelSendError;
use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::Message;
use ftl_types::message::MessageOrSignal;
use ftl_types::signal::Signal;
use serde::Deserializer;

use crate::handle::Handle;
use crate::sync::Arc;

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

#[derive(Clone)]
pub struct Sender {
    channel: Arc<Channel>,
}

impl Sender {
    pub fn handle_id(&self) -> HandleId {
        self.channel.handle_id()
    }

    pub fn send(&self, message: Message) -> Result<(), SendError> {
        self.channel.send(message)
    }

    pub fn notify(&self, signal: Signal) -> Result<(), FtlError> {
        self.channel.notify(signal)
    }
}

pub struct Receiver {
    channel: Arc<Channel>,
}

impl Receiver {
    pub fn handle_id(&self) -> HandleId {
        self.channel.handle_id()
    }

    pub fn receive(&self) -> Result<MessageOrSignal, FtlError> {
        self.channel.receive()
    }
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

    pub fn split(self) -> (Sender, Receiver) {
        let channel = Arc::new(self);
        let sender = Sender {
            channel: channel.clone(),
        };
        let receiver = Receiver { channel };

        (sender, receiver)
    }

    pub(crate) fn kernel_raw(&mut self) -> &mut ftl_kernel::channel::Channel {
        &mut self.raw
    }

    pub fn send(&self, message: Message) -> Result<(), SendError> {
        match self.raw.send(message) {
            Ok(()) => Ok(()),
            Err(KernelSendError { error, message }) => Err(SendError { error, message }),
        }
    }

    pub fn notify(&self, signal: Signal) -> Result<(), FtlError> {
        self.raw.notify(signal)
    }

    pub fn receive(&self) -> Result<MessageOrSignal, FtlError> {
        self.raw.receive()
    }

    pub fn call(&self, message: Message) -> Result<MessageOrSignal, CallError> {
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

    let handle = Handle::new(HandleId::from_isize(handle_id));
    let ch = Channel::from_handle(handle);
    Ok(ch)
}
