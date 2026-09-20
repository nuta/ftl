use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;

use crate::arch::syscall0;
use crate::arch::syscall2;
use crate::arch::syscall3;
use crate::handle::OwnedHandle;
use crate::poll::Poll;

// TODO: Move this to the Console struct. Should we guarantee handle_id=1 == console,
//       like UNIX stdout?
pub fn write(buf: &[u8]) -> Result<usize, ErrorCode> {
    let len = syscall2(Syscall::ConsoleWrite, buf.as_ptr() as usize, buf.len())?;
    Ok(len)
}

pub struct Console {
    handle: OwnedHandle,
}

impl Console {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn open() -> Result<Self, ErrorCode> {
        let id = syscall0(Syscall::ConsoleOpen)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn id(&self) -> HandleId {
        self.handle.id()
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        syscall3(
            Syscall::ConsoleRead,
            self.handle.id().as_usize(),
            buf.as_mut_ptr() as usize,
            buf.len(),
        )
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        syscall2(
            Syscall::ConsoleSubscribe,
            self.handle.id().as_usize(),
            poll.handle().id().as_usize(),
        )?;
        Ok(())
    }
}
