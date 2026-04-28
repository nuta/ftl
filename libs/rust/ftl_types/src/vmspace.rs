use core::ops::BitAnd;
use core::ops::BitAndAssign;
use core::ops::BitOr;
use core::ops::BitOrAssign;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageAttrs(usize);

impl PageAttrs {
    pub const NONE: Self = Self(0);
    pub const READABLE: Self = Self(0); // Pages are readable by default.
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

/// This is a marker trait for types that can be copied to user space.
///
/// - Must be `Copy`.
///
/// - Must not have padding. They might leak kernel memory to user space. This
///   can't be checked easily at compile time, and it's your responsibility to
///   ensure that.
pub unsafe trait UserCopyable: Copy {}

unsafe impl UserCopyable for u8 {}
unsafe impl UserCopyable for usize {}
unsafe impl<T: UserCopyable, const N: usize> UserCopyable for [T; N] {}
