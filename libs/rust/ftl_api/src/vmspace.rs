use core::ops::BitOr;

use crate::handle::Handle;
use crate::start::start_info;
use crate::vmarea::VmArea;

pub struct VmSpace {
    handle: Handle,
}

impl VmSpace {
    pub fn create() -> crate::Result<Self> {
        let start_info = start_info();
        let handle = (start_info.vmspace_create)()?;
        Ok(Self { handle })
    }

    pub fn map(&self, vmarea: &VmArea, uaddr: usize, attrs: PageAttrs) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.vmspace_map)(&self.handle, vmarea.handle(), uaddr, attrs)
    }

    pub fn read_bytes(&self, uaddr: usize, buf: &mut [u8]) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.vmspace_read)(&self.handle, uaddr, buf)
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Drop for VmSpace {
    fn drop(&mut self) {
        // TODO: Add vmspace_destroy supercall
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageAttrs(usize);

impl PageAttrs {
    // X64 PTE flags.
    #[cfg(target_arch = "x86_64")]
    pub const READ: Self = Self(1 << 0); // TODO: This is P bit actually. Should we use 0?
    #[cfg(target_arch = "x86_64")]
    pub const WRITE: Self = Self(1 << 1);
    #[cfg(target_arch = "x86_64")]
    pub const EXEC: Self = Self(1 << 2);

    // Host environment page attributes.
    #[cfg(not(target_os = "none"))]
    pub const READ: Self = Self(1 << 0);
    #[cfg(not(target_os = "none"))]
    pub const WRITE: Self = Self(1 << 1);
    #[cfg(not(target_os = "none"))]
    pub const EXEC: Self = Self(1 << 2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[cfg(feature = "kernel")]
    pub const fn as_raw(self) -> u64 {
        self.0 as u64
    }
}

impl BitOr for PageAttrs {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
