use crate::vmspace::UserCopyable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct HandleId(usize);

impl HandleId {
    /// An invalid ID.
    pub const ZERO: Self = Self::from_raw(0);

    pub const fn from_raw(id: usize) -> Self {
        Self(id)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

// SAFETY: The `HandleId` does not have padding.
unsafe impl UserCopyable for HandleId {}
