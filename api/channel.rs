use ftl_kernel::channel::CallError as KernelCallError;
use ftl_kernel::channel::SendError as KernelSendError;
use ftl_types::{error::FtlError, Message};

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
    raw: ftl_kernel::channel::Channel,
}

impl Channel {
    pub fn send(&mut self, message: Message) -> Result<(), SendError> {
        match self.raw.send(message) {
            Ok(()) => Ok(()),
            Err(KernelSendError { error, message }) => Err(SendError { error, message }),
        }
    }

    pub fn receive(&mut self) -> Result<Message, FtlError> {
        self.raw.receive()
    }

    pub fn call(&mut self, message: Message) -> Result<Message, CallError> {
        match self.raw.call(message) {
            Ok(message) => Ok(message),
            Err(KernelCallError::SendError(KernelSendError { error, message })) => {
                Err(CallError::SendError(SendError { error, message }))
            }
            Err(KernelCallError::ReceiveError(error)) => Err(CallError::ReceiveError(error)),
        }
    }
}
