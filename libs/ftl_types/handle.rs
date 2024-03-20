use alloc::string::String;
use core::fmt;
use core::num::NonZeroIsize;

/// A handle ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(NonZeroIsize);

impl HandleId {
    pub const fn from_nonzero(id: NonZeroIsize) -> HandleId {
        Self(id)
    }
}

/// Allowed operations on a handle.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleRights(u8);

impl HandleRights {
    const ZEROED: HandleRights = HandleRights(0x00);
    const ALL: HandleRights = HandleRights(0xff);
    pub const READABLE: HandleRights = HandleRights(1 << 0);
    pub const WRITABLE: HandleRights = HandleRights(1 << 1);

    pub const fn new_zeroed() -> HandleRights {
        Self::ZEROED
    }

    pub const fn new_all() -> HandleRights {
        Self::ALL
    }

    pub fn has(&self, rights: HandleRights) -> bool {
        (self.0 & rights.0) == rights.0
    }

    pub fn set(&mut self, rights: HandleRights) {
        self.0 |= rights.0;
    }

    pub fn clear(&mut self, rights: HandleRights) {
        self.0 &= !rights.0;
    }
}

impl fmt::Debug for HandleRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();

        s.push(if self.has(HandleRights::READABLE) {
            'R'
        } else {
            '-'
        });

        s.push(if self.has(HandleRights::WRITABLE) {
            'W'
        } else {
            '-'
        });

        write!(f, "{}", s)
    }
}
