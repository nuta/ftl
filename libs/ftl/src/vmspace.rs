use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::vmspace::PageAttrs;

use crate::arch::syscall1;
use crate::arch::syscall4;
use crate::handle::OwnedHandle;
use crate::vmo::Vmo;

pub struct VmSpace {
    handle: OwnedHandle,
}

impl VmSpace {
    pub const unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn try_clone(&self) -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::VmSpaceClone, self.handle.id().as_usize())?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn map(&self, vmo: &Vmo, uaddr: usize, attrs: PageAttrs) -> Result<(), ErrorCode> {
        syscall4(
            Syscall::VmSpaceMap,
            self.handle.id().as_usize(),
            vmo.handle().id().as_usize(),
            uaddr,
            attrs.as_raw(),
        )?;
        Ok(())
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
