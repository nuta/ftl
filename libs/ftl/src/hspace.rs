use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::syscall::Syscall;

use crate::arch::syscall0;
use crate::arch::syscall3;
use crate::handle::OwnedHandle;

/// A handle space.
pub struct HandleSpace {
    handle: OwnedHandle,
}

impl HandleSpace {
    pub const unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn into_handle(self) -> OwnedHandle {
        self.handle
    }

    pub fn create() -> Result<Self, ErrorCode> {
        let id = syscall0(Syscall::HSpaceCreate)?;
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn move_handle(
        &self,
        handle: OwnedHandle,
        rights: HandleRight,
    ) -> Result<HandleId, (ErrorCode, OwnedHandle)> {
        let id = match syscall3(
            Syscall::HSpaceMove,
            self.handle.id().as_usize(),
            handle.id().as_usize(),
            rights.as_raw(),
        ) {
            Ok(id) => id,
            Err(err) => return Err((err, handle)),
        };

        core::mem::forget(handle);
        Ok(HandleId::new(id))
    }

    pub fn duplicate(&self, rights: HandleRight) -> Result<Self, ErrorCode> {
        let handle = self.handle.duplicate(rights)?;
        Ok(Self { handle })
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
