use core::mem;

use ftl_types::{error::FtlError, handle::HandleId};

use crate::syscall;

pub struct MessageHeader(usize);

/// An owned handle, which will be closed when dropped.
///
/// # Invariant: `OwnedHandle` can be transmuted to `HandleId`
///
/// This type is marked as `#[repr(transparent)]` to ensure that it can be
/// transmuted to a `HandleId`. Some code depend on this fact so don't change
/// the sturcture of this type!
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct OwnedHandle(HandleId);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        syscall::handle_close(self.0);
    }
}

pub struct SendError {
    pub error: FtlError,
    // handles: ArrayVec<OwnedHandle>,
}

pub struct Channel {
    handle: HandleId,
}

impl Channel {
    pub fn create() -> Result<(Channel, Channel), FtlError> {
        let (handle0, handle1) = syscall::channel_create()?;
        let ch0 = Channel { handle: handle0 };
        let ch1 = Channel { handle: handle1 };
        Ok((ch0, ch1))
    }

    pub fn send(&self, header: MessageHeader, buf: &[u8], handles: &[OwnedHandle]) -> Result<(), SendError> {
        // SAFETY: `OwnedHandle` is `repr(transparent)` of `HandleId`. That is,
        //         they have the same memory layout.
        let handle_ids = unsafe { mem::transmute::<&[OwnedHandle], &[HandleId]>(handles) };
        match syscall::channel_send(self.handle, header.0, buf, handle_ids) {
            Ok(()) => {
                mem::forget(handles);
                Ok(())
            }
            Err(error) => {
                // Failed to send message. Since it's guaranteed that we still
                // own handles,
                Err(SendError { error })
            }
        }
    }
}

impl Into<OwnedHandle> for Channel {
    fn into(self) -> OwnedHandle {
        let owned = OwnedHandle(self.handle);

        // Prevent the handle from being closed when the channel is dropped.
        mem::forget(self);

        owned
    }
}
