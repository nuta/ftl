use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;

use crate::arch::syscall0;
use crate::arch::syscall3;
use crate::arch::syscall4;
use crate::arch::syscall6;
use crate::handle::OwnedHandle;
use crate::poll::Poll;

pub struct Nic {
    handle: OwnedHandle,
}

impl Nic {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn into_handle(self) -> OwnedHandle {
        self.handle
    }

    pub fn create_virtual() -> Result<Self, ErrorCode> {
        let id = syscall0(Syscall::NicCreateVirtual)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn id(&self) -> HandleId {
        self.handle.id()
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::NicSubscribe,
            self.handle.id().as_usize(),
            poll.handle().id().as_usize(),
            cookie,
        )?;
        Ok(())
    }

    pub fn bind(&self, port_start: u16, len: u16, dst: &Nic) -> Result<u16, ErrorCode> {
        let port_start = syscall4(
            Syscall::NicBind,
            self.handle.id().as_usize(),
            port_start as usize,
            len as usize,
            dst.handle.id().as_usize(),
        )?;
        Ok(port_start as u16)
    }

    pub fn unbind(&self, port_start: u16, len: u16) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::NicUnbind,
            self.handle.id().as_usize(),
            port_start as usize,
            len as usize,
        )?;
        Ok(())
    }

    pub fn recv(&self, header: &mut [u8], payload: &mut [u8]) -> Result<usize, ErrorCode> {
        syscall6(
            Syscall::NicRecv,
            self.handle.id().as_usize(),
            0,
            header.as_mut_ptr() as usize,
            header.len(),
            payload.as_mut_ptr() as usize,
            payload.len(),
        )
    }

    pub fn send(&self, header: &[u8], payload: &[u8]) -> Result<(), ErrorCode> {
        syscall6(
            Syscall::NicSend,
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
