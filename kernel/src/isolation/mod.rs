use ftl_types::error::ErrorCode;

use crate::shared_ref::RefCounted;
use crate::shared_ref::SharedRef;

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

    pub const fn len(&self) -> usize {
        self.end.0 - self.start.0
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

pub trait Isolation: Send + Sync {
    fn read_bytes(&self, buf: &mut [u8], slice: UserSlice) -> Result<(), ErrorCode>;
}

pub static INKERNEL_ISOLATION: SharedRef<dyn Isolation> = {
    static INNER: RefCounted<InKernelIsolation> = RefCounted::new(InKernelIsolation::new());
    let isolation = SharedRef::new_static(&INNER);
    isolation as SharedRef<dyn Isolation>
};

struct InKernelIsolation {
    _private: (),
}

impl InKernelIsolation {
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl Isolation for InKernelIsolation {
    fn read_bytes(&self, buf: &mut [u8], slice: UserSlice) -> Result<(), ErrorCode> {
        let ptr = slice.start.0 as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), slice.len());
        }
        Ok(())
    }
}
