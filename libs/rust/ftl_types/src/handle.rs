use core::ops::BitOr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleId(usize);

impl HandleId {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

/// A set of allowed actions on a kernel object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleRight(usize);

impl HandleRight {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const MAP: Self = Self(1 << 2);

    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOr for HandleRight {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
