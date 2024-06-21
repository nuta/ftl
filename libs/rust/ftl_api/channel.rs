use core::mem;

use ftl_inlinedvec::InlinedVec;
use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall;

pub struct MessageHeader(usize);

pub struct SendError {
    pub error: FtlError,
    pub handles: InlinedVec<OwnedHandle, 4>,
}

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
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

    pub fn send(
        &self,
        header: MessageHeader,
        buf: &[u8],
        handles: InlinedVec<OwnedHandle, 4>,
    ) -> Result<(), SendError> {
        // SAFETY: `OwnedHandle` is `repr(transparent)` of `HandleId`. That is,
        //         they have the same memory layout.
        let handle_ids =
            unsafe { mem::transmute::<&[OwnedHandle], &[HandleId]>(handles.as_slice()) };

        match syscall::channel_send(self.handle.id(), header.0, buf, handle_ids) {
            Ok(()) => {
                // We've successfully transferred the handles. Prevent them
                // from being closed.
                mem::forget(handles);

                Ok(())
            }
            Err(error) => {
                // Failed to send message. Since it's guaranteed that we still
                // own handles, return them back to the caller.
                Err(SendError { error, handles })
            }
        }
    }
}

impl Handleable for Channel {
    fn handle_id(&self) -> HandleId {
        self.handle.id()
    }
}
