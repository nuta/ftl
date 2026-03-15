use ftl_types::error::ErrorCode;
use ftl_types::vmspace::PageAttrs;

use crate::shared_ref::SharedRef;
use crate::vmspace::PageIter;
use crate::vmspace::VmSpace;

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

    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

/// A slice in an isolation space.
#[derive(Clone, Copy)]
pub struct UserSlice {
    pub start: UserPtr,
    pub end: UserPtr,
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
    fn vmspace(&self) -> &SharedRef<VmSpace>;
    fn is_inkernel(&self) -> bool;
    fn read_bytes(&self, slice: &UserSlice, buf: &mut [u8]) -> Result<(), ErrorCode>;
    fn write_bytes(&self, slice: &UserSlice, buf: &[u8]) -> Result<(), ErrorCode>;
}

pub struct IdleIsolation {
    _private: (),
}

impl IdleIsolation {
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl Isolation for IdleIsolation {
    fn vmspace(&self) -> &SharedRef<VmSpace> {
        unreachable!()
    }

    fn is_inkernel(&self) -> bool {
        true
    }

    fn read_bytes(&self, _slice: &UserSlice, _buf: &mut [u8]) -> Result<(), ErrorCode> {
        unreachable!()
    }

    fn write_bytes(&self, _slice: &UserSlice, _buf: &[u8]) -> Result<(), ErrorCode> {
        unreachable!()
    }
}

pub struct InKernelIsolation {
    vmspace: SharedRef<VmSpace>,
}

impl InKernelIsolation {
    pub fn new(vmspace: SharedRef<VmSpace>) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self { vmspace })
    }
}

impl Isolation for InKernelIsolation {
    fn vmspace(&self) -> &SharedRef<VmSpace> {
        &self.vmspace
    }

    fn is_inkernel(&self) -> bool {
        true
    }

    fn read_bytes(&self, slice: &UserSlice, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() != slice.len() {
            return Err(ErrorCode::InvalidArgument);
        }

        let mut offset = 0;
        for page in PageIter::new(&self.vmspace, slice) {
            let src = page.slice(PageAttrs::NONE)?;
            buf[offset..offset + src.len()].copy_from_slice(src);
            offset += src.len();
        }
        Ok(())
    }

    fn write_bytes(&self, slice: &UserSlice, buf: &[u8]) -> Result<(), ErrorCode> {
        if buf.len() != slice.len() {
            return Err(ErrorCode::InvalidArgument);
        }

        let mut offset = 0;
        for page in PageIter::new(&self.vmspace, slice) {
            let dst = page.slice(PageAttrs::WRITABLE)?;
            dst.copy_from_slice(&buf[offset..offset + dst.len()]);
            offset += dst.len();
        }
        Ok(())
    }
}

pub struct SandboxIsolation {
    vmspace: SharedRef<VmSpace>,
}

impl SandboxIsolation {
    pub fn new(vmspace: SharedRef<VmSpace>) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self { vmspace })
    }
}

impl Isolation for SandboxIsolation {
    fn vmspace(&self) -> &SharedRef<VmSpace> {
        &self.vmspace
    }

    fn is_inkernel(&self) -> bool {
        false
    }

    fn read_bytes(&self, _slice: &UserSlice, _buf: &mut [u8]) -> Result<(), ErrorCode> {
        // System calls from a sandboxed process won't be handled by the kernel.
        unreachable!()
    }

    fn write_bytes(&self, _slice: &UserSlice, _buf: &[u8]) -> Result<(), ErrorCode> {
        // System calls from a sandboxed process won't be handled by the kernel.
        unreachable!()
    }
}
