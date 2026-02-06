use core::fmt;

use ftl_types::error::ErrorCode;
// TODO: Make this private
pub use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_HANDLE_CLOSE;
use log::trace;

use crate::syscall::syscall1;

pub struct OwnedHandle(HandleId);

impl OwnedHandle {
    // TODO: Make this private
    pub const fn from_raw(id: HandleId) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> HandleId {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        trace!("dropping handle: {:?}", self);
        if let Err(error) = sys_handle_close(self.0) {
            trace!("failed to close handle: {:?}", error);
        }
    }
}

impl fmt::Debug for OwnedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedHandle")
            .field(&self.0.as_usize())
            .finish()
    }
}

pub trait Handleable {
    fn handle(&self) -> &OwnedHandle;
}

pub fn sys_handle_close(id: HandleId) -> Result<(), ErrorCode> {
    syscall1(SYS_HANDLE_CLOSE, id.as_usize())?;
    Ok(())
}
