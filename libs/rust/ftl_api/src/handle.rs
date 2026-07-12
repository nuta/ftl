use core::any::TypeId;
use core::ops::BitOr;

/// A reference to a kernel object, with allowed actions.
pub struct Handle {
    /// An opaque pointer to the kernel memory.
    raw: usize,
    /// The type of the kernel object.
    type_id: TypeId,
    /// The rights granted to the handle. Also known as *capability*.
    right: HandleRight,
}

impl Handle {
    #[cfg(feature = "kernel")]
    pub fn new<T: 'static>(raw: usize, right: HandleRight) -> Self {
        Self {
            raw,
            type_id: TypeId::of::<T>(),
            right,
        }
    }

    #[cfg(feature = "kernel")]
    pub fn raw(&self) -> usize {
        self.raw
    }

    #[cfg(feature = "kernel")]
    pub fn is_type<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    pub fn authorize(&self, action: HandleRight) -> bool {
        self.right.contains(action)
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
