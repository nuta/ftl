use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::vmo::SupplyMode;

use crate::arch::syscall1;
use crate::arch::syscall3;
use crate::arch::syscall4;
use crate::arch::syscall6;
use crate::handle::OwnedHandle;

pub struct Vmo {
    handle: OwnedHandle,
}

impl Vmo {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn create_zeroed(len: usize) -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::VmoCreateZeroed, len)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    /// Creates a user-paged VMO.
    ///
    /// When a thread accesses a page in the VMO for the first time, a page
    /// fault will be sent to the userspace page fault handler, which can
    /// fill the page programmatically by calling [`Self::supply`].
    pub fn create_user(len: usize) -> Result<Self, ErrorCode> {
        let id = syscall1(Syscall::VmoCreateUser, len)?;
        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    /// Creates a snapshot of the VMO from `offset` to `offset + len`.
    ///
    /// The kernel copies present pages in the range into a new VMO.
    ///
    /// The newly created VMO will be `len` bytes long, and its first byte is
    /// the same as the original VMO at `offset`.
    pub fn snapshot(&self, offset: usize, len: usize) -> Result<Self, ErrorCode> {
        let id = syscall3(
            Syscall::VmoSnapshot,
            self.handle.id().as_usize(),
            offset,
            len,
        )?;

        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        let mut written = 0;
        while written < buf.len() {
            let rest = &buf[written..];
            written += syscall4(
                Syscall::VmoWrite,
                self.handle.id().as_usize(),
                offset + written,
                rest.as_ptr() as usize,
                rest.len(),
            )?;
        }

        Ok(())
    }

    /// Fills absent pages in a user-paged VMO with `buf`.
    ///
    /// Present pages are skipped. `offset` and `buf.len()` must be
    /// page-aligned.
    pub fn supply(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        let mut total_len = 0;
        while total_len < buf.len() {
            let rest = &buf[total_len..];
            total_len += syscall6(
                Syscall::VmoSupply,
                self.handle.id().as_usize(),
                offset + total_len,
                SupplyMode::Copy as usize,
                rest.as_ptr() as usize,
                rest.len(),
                0,
            )?;
        }

        Ok(())
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
