use core::num::NonZeroIsize;

/// A handle ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(NonZeroIsize);

impl HandleId {
    pub const fn from_raw(id: isize) -> Option<HandleId> {
        match NonZeroIsize::new(id) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    pub const fn from_nonzero(id: NonZeroIsize) -> HandleId {
        Self(id)
    }

    pub const fn into_raw(self) -> isize {
        self.0.get()
    }
}

/// Allowed operations on a handle.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleRights(pub u8);

impl HandleRights {
    pub const NONE: HandleRights = HandleRights(0);
}
