use ftl_types::error::FtlError;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;

use crate::handle::OwnedHandle;
use crate::syscall;

pub enum RecvError {
    KernelError(FtlError),
    UnexpectedMessageType(MessageInfo),
}

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
    pub fn from_handle(handle: OwnedHandle) -> Channel {
        Channel { handle }
    }

    pub fn create() -> Result<(Channel, Channel), FtlError> {
        let (handle0, handle1) = syscall::channel_create()?;
        let ch0 = Channel {
            handle: OwnedHandle::from_raw(handle0),
        };
        let ch1 = Channel {
            handle: OwnedHandle::from_raw(handle1),
        };

        Ok((ch0, ch1))
    }

    pub fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    pub fn send(&self, msginfo: MessageInfo, msg: &MessageBuffer) -> Result<(), FtlError> {
        syscall::channel_send(self.handle.id(), msginfo, msg as *const _ as *const u8)
    }

    pub fn recv(&self, msg: &mut MessageBuffer) -> Result<MessageInfo, FtlError> {
        syscall::channel_recv(self.handle.id(), msg as *mut _ as *mut u8)
    }

    pub fn recv2<T>(&self, msg: &mut MessageBuffer) -> Result<T, RecvError> {
        let msginfo = self.recv(msg).map_err(RecvError::KernelError)?;
        if msginfo != T::MSGINFO {
            todo!("free received handles here");
            return Err(RecvError::UnexpectedMessageType(msginfo));
        }

        todo!()
    }
}
