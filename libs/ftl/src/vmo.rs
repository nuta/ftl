use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;
use crate::arch::syscall4;
use crate::handle::OwnedHandle;

pub struct Vmo {
    handle: OwnedHandle,
}

impl Vmo {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn create(len: usize) -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::VmoCreate, len)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        syscall4(
            Syscall::VmoWrite,
            self.handle.id().as_usize(),
            offset,
            buf.as_ptr() as usize,
            buf.len(),
        )?;
        Ok(())
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
