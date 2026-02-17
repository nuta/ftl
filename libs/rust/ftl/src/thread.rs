use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_THREAD_CREATE;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::process::Process;
use crate::syscall::syscall4;

pub struct Thread {
    handle: OwnedHandle,
}

impl Thread {
    pub fn create(
        process: &Process,
        entry: usize,
        sp: usize,
        start_info: usize,
    ) -> Result<Self, ErrorCode> {
        let handle = sys_thread_create(process.handle().id(), entry, sp, start_info)?;
        Ok(Self { handle })
    }
}

impl Handleable for Thread {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Thread")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

pub fn sys_thread_create(
    process: HandleId,
    entry: usize,
    sp: usize,
    start_info: usize,
) -> Result<OwnedHandle, ErrorCode> {
    let id = syscall4(
        SYS_THREAD_CREATE,
        process.as_usize(),
        entry,
        sp as usize,
        start_info,
    )?;
    let handle = OwnedHandle::from_raw(HandleId::from_raw(id));
    Ok(handle)
}
