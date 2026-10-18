use core::ops::BitOr;
use core::ops::BitOrAssign;

use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;

use crate::error::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAttrs(usize);

impl PageAttrs {
    // Permission bits, translated into hardware flags by the kernel.
    pub const EMPTY: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXEC: Self = Self(1 << 2);

    pub const ALL: Self = Self(Self::READ.0 | Self::WRITE.0 | Self::EXEC.0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn as_raw(self) -> usize {
        self.0
    }

    pub const fn from_raw(raw: usize) -> Result<Self, ErrorCode> {
        if raw & !Self::ALL.0 != 0 {
            // Unknown bits are set.
            return Err(ErrorCode::InvalidPageAttrs);
        }

        Ok(Self(raw))
    }

    pub const fn from_raw_truncated(raw: usize) -> Self {
        Self(raw & Self::ALL.0)
    }

    pub const fn from_phdr_flags(flags: u32) -> Self {
        let mut attrs = Self::EMPTY;
        if flags & PF_R != 0 {
            attrs.0 |= Self::READ.0;
        }
        if flags & PF_W != 0 {
            attrs.0 |= Self::WRITE.0;
        }
        if flags & PF_X != 0 {
            attrs.0 |= Self::EXEC.0;
        }
        attrs
    }
}

impl BitOr for PageAttrs {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PageAttrs {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
