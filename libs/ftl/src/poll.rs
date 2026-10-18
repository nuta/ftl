use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::time::MonoTime;

use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::handle::OwnedHandle;

pub struct Poll {
    handle: OwnedHandle,
}

impl Poll {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn into_handle(self) -> OwnedHandle {
        self.handle
    }

    pub fn create() -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::PollCreate, 0)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn wait(&self) -> Result<usize, ErrorCode> {
        syscall1(Syscall::PollWait, self.handle.id().as_usize())
    }

    pub fn wait_until(&self, deadline: MonoTime) -> Result<usize, ErrorCode> {
        syscall2(
            Syscall::PollWaitUntil,
            self.handle.id().as_usize(),
            &deadline as *const MonoTime as usize,
        )
    }

    pub fn notify(&self, cookie: usize) -> Result<(), ErrorCode> {
        syscall2(Syscall::PollNotify, self.handle.id().as_usize(), cookie)?;
        Ok(())
    }

    pub const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
