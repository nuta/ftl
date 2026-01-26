use core::fmt;

use ftl_utils::alignment::is_aligned;

/// A virtual memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
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

/// A physical memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PAddr(usize);

impl PAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
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
