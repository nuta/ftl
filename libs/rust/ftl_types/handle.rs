use core::num::NonZeroI32;

/// A handle ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(NonZeroI32);

impl HandleId {
    pub const fn from_raw(id: i32) -> Option<HandleId> {
        match NonZeroI32::new(id) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    pub const fn from_nonzero(id: NonZeroI32) -> HandleId {
        Self(id)
    }

    pub const fn as_isize(self) -> isize {
        self.0.get() as isize
    }

    pub const fn as_i32(self) -> i32 {
        self.0.get()
    }
}

/// Allowed operations on a handle.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleRights(pub u8);

impl HandleRights {
    pub const NONE: HandleRights = HandleRights(0);
}
