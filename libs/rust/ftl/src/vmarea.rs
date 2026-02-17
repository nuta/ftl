use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_VMAREA_CREATE;
use ftl_types::syscall::SYS_VMAREA_READ;
use ftl_types::syscall::SYS_VMAREA_WRITE;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall4;

pub struct VmArea {
    handle: OwnedHandle,
}

impl VmArea {
    pub fn new() -> Result<Self, ErrorCode> {
        let handle = sys_vmarea_create()?;
        Ok(Self { handle })
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), ErrorCode> {
        sys_vmarea_read(self.handle.id(), offset, buf)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        sys_vmarea_write(self.handle.id(), offset, buf)
    }
}

impl Handleable for VmArea {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

impl fmt::Debug for VmArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VmArea")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

fn sys_vmarea_create() -> Result<OwnedHandle, ErrorCode> {
    let raw = syscall0(SYS_VMAREA_CREATE)?;
    let handle = OwnedHandle::from_raw(HandleId::from_raw(raw));
    Ok(handle)
}

fn sys_vmarea_read(vmarea: HandleId, offset: usize, buf: &mut [u8]) -> Result<(), ErrorCode> {
    syscall4(
        SYS_VMAREA_READ,
        vmarea.as_usize(),
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )?;
    Ok(())
}

fn sys_vmarea_write(vmarea: HandleId, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
    syscall4(
        SYS_VMAREA_WRITE,
        vmarea.as_usize(),
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )?;
    Ok(())
}
