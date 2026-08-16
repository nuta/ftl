use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_utils::alignment::is_aligned;

use crate::arch::USER_ADDR_END;

/// A physical memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PAddr(usize);

impl PAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn is_aligned(self, alignment: usize) -> bool {
        is_aligned(self.0, alignment)
    }
}

impl fmt::Debug for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A kernel virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
}

impl fmt::Debug for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A user virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct UAddr(usize);

impl UAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn add(self, offset: usize) -> Option<Self> {
        match self.0.checked_add(offset) {
            Some(addr) => Some(Self(addr)),
            None => None,
        }
    }

    pub fn is_aligned_to(self, alignment: usize) -> bool {
        is_aligned(self.0, alignment)
    }
}

impl fmt::Debug for UAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for UAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy)]
pub struct USlice {
    addr: UAddr,
    len: usize,
}

impl USlice {
    pub const fn new(addr: UAddr, len: usize) -> Result<Self, ErrorCode> {
        let Some(end) = addr.add(len) else {
            return Err(ErrorCode::INVALID_ARG);
        };

        if end.as_usize() > USER_ADDR_END {
            return Err(ErrorCode::NOT_ALLOWED);
        }

        Ok(Self { addr, len })
    }

    pub fn read(self, dst: &mut [u8]) -> Result<(), ErrorCode> {
        if self.len != dst.len() {
            return Err(ErrorCode::INVALID_ARG);
        }

        unsafe {
            crate::arch::usercopy_read(self.addr, dst.as_mut_ptr(), self.len)?;
        }

        Ok(())
    }
}
