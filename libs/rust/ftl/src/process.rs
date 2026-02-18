use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_PROCESS_CREATE_SANDBOXED;
use ftl_types::syscall::SYS_PROCESS_EXIT;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall3;
use crate::vmspace::VmSpace;

pub struct Process {
    handle: OwnedHandle,
}

impl Process {
    pub fn create_sandboxed(vmspace: &VmSpace, name: &str) -> Result<Self, ErrorCode> {
        let handle = sys_process_create_sandboxed(vmspace.handle().id(), name)?;
        Ok(Self { handle })
    }
}

impl Handleable for Process {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

impl fmt::Debug for Process {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Process")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

pub fn sys_process_create_sandboxed(
    vmspace: HandleId,
    name: &str,
) -> Result<OwnedHandle, ErrorCode> {
    let raw = syscall3(
        SYS_PROCESS_CREATE_SANDBOXED,
        vmspace.as_usize(),
        name.as_ptr() as usize,
        name.len(),
    )?;

    let handle = OwnedHandle::from_raw(HandleId::from_raw(raw));
    Ok(handle)
}

pub fn process_exit() -> ! {
    let _ = syscall0(SYS_PROCESS_EXIT);
    unreachable!();
}
