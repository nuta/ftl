use core::ops::BitAnd;
use core::ops::BitAndAssign;
use core::ops::BitOr;
use core::ops::BitOrAssign;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAttrs(usize);

impl PageAttrs {
    pub const NONE: Self = Self(0);
    #[cfg(target_arch = "x86_64")]
    pub const READABLE: Self = Self(0); // Pages are readable by default.
    #[cfg(target_arch = "x86_64")]
    pub const WRITABLE: Self = Self(1 << 1); // PTE_W

    pub const fn from_raw(attrs: usize) -> Self {
        Self(attrs)
    }

    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr<Self> for PageAttrs {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<Self> for PageAttrs {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd<Self> for PageAttrs {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign<Self> for PageAttrs {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
