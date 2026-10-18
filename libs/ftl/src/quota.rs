use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::quota::Resource;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;
use crate::arch::syscall3;
use crate::handle::OwnedHandle;

pub struct Quota {
    handle: OwnedHandle,
}

impl Quota {
    pub const unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn into_handle(self) -> OwnedHandle {
        self.handle
    }

    pub fn create(parent: &Quota) -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::QuotaCreate, parent.handle.id().as_usize())?;
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn limit(&self, resource: Resource, limit: usize) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::QuotaLimit,
            self.handle.id().as_usize(),
            resource as usize,
            limit,
        )?;
        Ok(())
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
