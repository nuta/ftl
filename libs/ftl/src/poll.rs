use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::poll::Event;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;
use crate::handle::OwnedHandle;

pub struct Poll {
    handle: OwnedHandle,
}

impl Poll {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn create() -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::PollCreate, 0)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn wait(&self) -> Result<Event, ErrorCode> {
        let event = syscall1(Syscall::PollWait, self.handle.id().as_usize())?;
        Ok(Event::from_raw(event as u32))
    }

    pub fn notify(&self) -> Result<(), ErrorCode> {
        syscall1(Syscall::PollNotify, self.handle.id().as_usize())?;
        Ok(())
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
