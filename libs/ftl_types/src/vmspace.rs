use core::ops::BitOr;
use core::ops::BitOrAssign;

#[derive(Debug, Clone, Copy)]
pub struct PageAttrs(usize);

impl PageAttrs {
    // Permission bits, translated into hardware flags by the kernel.
    pub const EMPTY: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXEC: Self = Self(1 << 2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn as_raw(self) -> usize {
        self.0
    }

    pub const fn from_raw(raw: usize) -> Self {
        Self(raw)
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
