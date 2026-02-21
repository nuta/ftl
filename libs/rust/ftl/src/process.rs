use core::fmt;
use core::mem;

pub use ftl_types::environ::PROCESS_NAME_MAX_LEN;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_PROCESS_CREATE_INKERNEL;
use ftl_types::syscall::SYS_PROCESS_CREATE_SANDBOXED;
use ftl_types::syscall::SYS_PROCESS_EXIT;
use ftl_types::syscall::SYS_PROCESS_INJECT_HANDLE;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall2;
use crate::syscall::syscall3;
use crate::vmspace::VmSpace;

pub struct Process {
    handle: OwnedHandle,
}

impl Process {
    pub fn create_inkernel(vmspace: &VmSpace, name: &str) -> Result<Self, ErrorCode> {
        let handle = sys_process_create_inkernel(vmspace.handle().id(), name)?;
        Ok(Self { handle })
    }

    pub fn create_sandboxed(vmspace: &VmSpace, name: &str) -> Result<Self, ErrorCode> {
        let handle = sys_process_create_sandboxed(vmspace.handle().id(), name)?;
        Ok(Self { handle })
    }

    pub fn inject_handle<H: Handleable>(&self, handle: H) -> Result<HandleId, ErrorCode> {
        let handle_id = handle.handle().id();
        let injected_id = sys_process_inject_handle(self.handle.id(), handle_id)?;

        // The kernel moved the handle into the target process.
        mem::forget(handle.into_handle());
        Ok(injected_id)
    }
}

impl Handleable for Process {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn into_handle(self) -> OwnedHandle {
        self.handle
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

pub fn sys_process_create_inkernel(
    vmspace: HandleId,
    name: &str,
) -> Result<OwnedHandle, ErrorCode> {
    let raw = syscall3(
        SYS_PROCESS_CREATE_INKERNEL,
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

fn sys_process_inject_handle(process: HandleId, handle: HandleId) -> Result<HandleId, ErrorCode> {
    let id = syscall2(
        SYS_PROCESS_INJECT_HANDLE,
        process.as_usize(),
        handle.as_usize(),
    )?;
    Ok(HandleId::from_raw(id))
}
