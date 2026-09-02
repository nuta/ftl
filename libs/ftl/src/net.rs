use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::net::Rule;
use ftl_types::syscall::Syscall;

use crate::arch::syscall0;
use crate::arch::syscall1;
use crate::arch::syscall2;
use crate::arch::syscall3;
use crate::arch::syscall6;
use crate::handle::OwnedHandle;
use crate::poll::Poll;

pub struct Net {
    handle: OwnedHandle,
}

impl Net {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn create() -> Result<Self, ErrorCode> {
        let id = syscall0(Syscall::NetCreate)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn id(&self) -> HandleId {
        self.handle.id()
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        syscall2(
            Syscall::NetSubscribe,
            self.handle.id().as_usize(),
            poll.handle().id().as_usize(),
        )?;
        Ok(())
    }

    pub fn bind(&self, rule: &Rule) -> Result<(), ErrorCode> {
        syscall2(
            Syscall::NetBind,
            self.handle.id().as_usize(),
            rule as *const Rule as usize,
        )?;
        Ok(())
    }

    pub fn unbind(&self, rule: &Rule) -> Result<(), ErrorCode> {
        syscall2(
            Syscall::NetUnbind,
            self.handle.id().as_usize(),
            rule as *const Rule as usize,
        )?;
        Ok(())
    }

    pub fn peek(&self, header: &mut [u8]) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::NetPeek,
            self.handle.id().as_usize(),
            header.as_mut_ptr() as usize,
            header.len(),
        )?;
        Ok(())
    }

    pub fn recv(&self, payload: &mut [u8]) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::NetRecv,
            self.handle.id().as_usize(),
            payload.as_mut_ptr() as usize,
            payload.len(),
        )?;
        Ok(())
    }

    pub fn drop(&self) -> Result<(), ErrorCode> {
        syscall1(Syscall::NetDrop, self.handle.id().as_usize())?;
        Ok(())
    }

    pub fn send(&self, header: &[u8], payload: &[u8]) -> Result<(), ErrorCode> {
        syscall6(
            Syscall::NetSend,
            self.handle.id().as_usize(),
            0,
            header.as_ptr() as usize,
            header.len(),
            payload.as_ptr() as usize,
            payload.len(),
        )?;
        Ok(())
    }
}
