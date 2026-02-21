use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_VMSPACE_CREATE;
use ftl_types::syscall::SYS_VMSPACE_MAP;
pub use ftl_types::vmspace::PageAttrs;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall4;
use crate::vmarea::VmArea;

pub struct VmSpace {
    handle: OwnedHandle,
}

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        let handle = sys_vmspace_create()?;
        Ok(Self { handle })
    }

    pub fn map(&self, vmarea: &VmArea, uaddr: usize, attrs: PageAttrs) -> Result<(), ErrorCode> {
        let vmarea_id = vmarea.handle().id();
        sys_vmspace_map(self.handle.id(), vmarea_id, uaddr, attrs)
    }
}

impl Handleable for VmSpace {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn into_handle(self) -> OwnedHandle {
        self.handle
    }
}

impl fmt::Debug for VmSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VmSpace")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

fn sys_vmspace_create() -> Result<OwnedHandle, ErrorCode> {
    let raw = syscall0(SYS_VMSPACE_CREATE)?;
    let handle = OwnedHandle::from_raw(HandleId::from_raw(raw));
    Ok(handle)
}

fn sys_vmspace_map(
    vmspace: HandleId,
    vmarea: HandleId,
    uaddr: usize,
    attrs: PageAttrs,
) -> Result<(), ErrorCode> {
    syscall4(
        SYS_VMSPACE_MAP,
        vmspace.as_usize(),
        vmarea.as_usize(),
        uaddr,
        attrs.as_usize(),
    )?;
    Ok(())
}
