use core::fmt;

use ftl_utils::alignment::is_aligned;

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
