use core::range::Range;

use ftl_types::error::ErrorCode;

/// A pointer in an isolation space.
///
/// This is opaque and isolation-specific. For example it is a raw pointer
/// in the user mode isolation, but it is a offset in WebAssembly isolation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct UserPtr(usize);

impl UserPtr {
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    pub const fn checked_add(&self, offset: usize) -> Option<Self> {
        match self.0.checked_add(offset) {
            Some(ptr) => Some(Self(ptr)),
            None => None,
        }
    }
}

/// A slice in an isolation space.
pub struct UserSlice {
    start: UserPtr,
    end: UserPtr,
}

impl UserSlice {
    pub const fn new(ptr: UserPtr, len: usize) -> Option<Self> {
        let Some(end) = ptr.checked_add(len) else {
            return None;
        };

        Some(Self { start: ptr, end })
    }

    pub const fn subslice(&self, offset: usize, len: usize) -> Option<Self> {
        let Some(start) = self.start.checked_add(offset) else {
            return None;
        };

        let Some(end) = start.checked_add(len) else {
            return None;
        };

        if end.0 > self.end.0 {
            return None;
        }

        Some(Self { start, end })
    }
}

pub trait Isolation {
    fn read_bytes(&self, buf: &mut [u8], slice: UserSlice) -> Result<(), ErrorCode>;
}
