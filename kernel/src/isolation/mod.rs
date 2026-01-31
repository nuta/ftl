use core::mem::MaybeUninit;

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
#[derive(Clone, Copy)]
pub struct UserSlice {
    start: UserPtr,
    end: UserPtr,
}

impl UserSlice {
    pub const fn new(ptr: UserPtr, len: usize) -> Result<Self, ErrorCode> {
        let Some(end) = ptr.checked_add(len) else {
            return Err(ErrorCode::OutOfBounds);
        };

        Ok(Self { start: ptr, end })
    }

    pub const fn len(&self) -> usize {
        self.end.0 - self.start.0
    }

    pub const fn subslice(&self, offset: usize, len: usize) -> Result<Self, ErrorCode> {
        let Some(start) = self.start.checked_add(offset) else {
            return Err(ErrorCode::OutOfBounds);
        };

        let Some(end) = start.checked_add(len) else {
            return Err(ErrorCode::OutOfBounds);
        };

        if end.0 > self.end.0 {
            return Err(ErrorCode::OutOfBounds);
        }

        Ok(Self { start, end })
    }
}

pub fn read<'a, T: Copy>(
    isolation: &SharedRef<dyn Isolation>,
    slice: &UserSlice,
    offset: usize,
) -> Result<T, ErrorCode> {
    debug_assert!(
        size_of::<T>() <= 256,
        "T is too large and will consume too much stack"
    );

    let mut buf = MaybeUninit::<T>::uninit();

    let subslice = slice.subslice(offset, size_of::<T>())?;
    let slice =
        unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, size_of::<T>()) };

    isolation.read_bytes(&subslice, slice)?;
    Ok(unsafe { buf.assume_init() })
}

pub fn write<T: Copy>(
    isolation: &SharedRef<dyn Isolation>,
    slice: &UserSlice,
    offset: usize,
    value: T,
) -> Result<(), ErrorCode> {
    let subslice = slice.subslice(offset, size_of::<T>())?;
    let bytes =
        unsafe { core::slice::from_raw_parts(&raw const value as *const u8, size_of::<T>()) };

    isolation.write_bytes(&subslice, bytes)
}

pub trait Isolation: Send + Sync {
    fn read_bytes(&self, slice: &UserSlice, buf: &mut [u8]) -> Result<(), ErrorCode>;
    fn write_bytes(&self, slice: &UserSlice, buf: &[u8]) -> Result<(), ErrorCode>;
}

pub static INKERNEL_ISOLATION: SharedRef<dyn Isolation> = {
    static INNER: RefCounted<InKernelIsolation> = RefCounted::new_static(InKernelIsolation::new());
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
    fn read_bytes(&self, slice: &UserSlice, buf: &mut [u8]) -> Result<(), ErrorCode> {
        let src = slice.start.0 as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), slice.len());
        }
        Ok(())
    }

    fn write_bytes(&self, slice: &UserSlice, buf: &[u8]) -> Result<(), ErrorCode> {
        let dst = slice.start.0 as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), dst, slice.len());
        }
        Ok(())
    }
}
