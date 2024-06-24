use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageInfo;
use ftl_types::message::MESSAGE_DATA_MAX_LEN;

use crate::handle::OwnedHandle;
use crate::message::MessageBuffer;
use crate::message::MessageType;
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

    pub fn send<T: MessageType>(&self, buffer: &mut MessageBuffer, msg: T) -> Result<(), FtlError> {
        buffer.write(msg);

        // TODO: return send error to keep owning handles
        syscall::channel_send(
            self.handle.id(),
            T::MSGINFO,
            buffer.data.as_ptr(),
            buffer.handles.as_ptr(),
        )
    }

    pub fn recv<T: MessageType>(&self, buffer: &mut MessageBuffer) -> Result<T, RecvError> {
        let msginfo = syscall::channel_recv(
            self.handle.id(),
            buffer.data.as_mut_ptr(),
            buffer.handles.as_mut_ptr(),
        )
        .map_err(RecvError::KernelError)?;

        // Is it really the message we're expecting?
        if msginfo != T::MSGINFO {
            // Close transferred handles to prevent resource leaks.
            //
            // Also, if they're IPC-related handles like channels, this might
            // let the sender know that we don't never use them. Otherwise, the
            // sender might be waiting for a message from us.
            for i in 0..msginfo.num_handles() {
                let handle_id = buffer.handles[i];
                syscall::handle_close(handle_id).expect("failed to close handle");
            }

            return Err(RecvError::UnexpectedMessageType(msginfo));
        }

        todo!()
    }
}
