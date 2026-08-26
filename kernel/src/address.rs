use core::fmt;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_types::error::ErrorCode;
use ftl_utils::alignment::is_aligned;

use crate::arch::USER_ADDR_END;
use crate::arch::usercopy_read;

/// A physical memory address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PAddr(usize);

impl PAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn is_aligned(self, alignment: usize) -> bool {
        is_aligned(self.0, alignment)
    }
}

impl fmt::Debug for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A kernel virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn add(self, offset: usize) -> Option<Self> {
        match self.0.checked_add(offset) {
            Some(addr) => Some(Self(addr)),
            None => None,
        }
    }
}

impl fmt::Debug for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A user virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct UAddr(usize);

impl UAddr {
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn add(self, offset: usize) -> Option<Self> {
        match self.0.checked_add(offset) {
            Some(addr) => Some(Self(addr)),
            None => None,
        }
    }

    pub fn is_aligned_to(self, alignment: usize) -> bool {
        is_aligned(self.0, alignment)
    }
}

impl fmt::Debug for UAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(target_pointer_width = "64") {
            write!(f, "0x{:016x}", self.0)
        } else {
            write!(f, "0x{:08x}", self.0)
        }
    }
}

impl fmt::Display for UAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy)]
pub struct USlice {
    addr: UAddr,
    end: UAddr,
}

impl USlice {
    pub const fn new(addr: UAddr, len: usize) -> Result<Self, ErrorCode> {
        let Some(end) = addr.add(len) else {
            return Err(ErrorCode::INVALID_ARG);
        };

        if end.as_usize() > USER_ADDR_END {
            return Err(ErrorCode::NOT_ALLOWED);
        }

        Ok(Self { addr, end })
    }

    pub const fn len(self) -> usize {
        self.end.as_usize() - self.addr.as_usize()
    }

    pub fn subslice(self, start: usize, len: usize) -> Result<Self, ErrorCode> {
        let Some(new_start) = self.addr.add(start) else {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        };
        let Some(new_end) = new_start.add(len) else {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        };

        if new_end.as_usize() > self.end.as_usize() {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        Ok(Self {
            addr: new_start,
            end: new_end,
        })
    }

    pub fn read_bytes(self, dst: &mut [u8]) -> Result<(), ErrorCode> {
        // SAFETY: &mut [u8] is a non-null pointer and carries the length.
        unsafe { self.read(dst.as_mut_ptr(), dst.len()) }
    }

    /// Reads the user address into a kernel's uninitialized buffer.
    ///
    /// # Safety
    ///
    /// The user slice must be a valid representation of `T`.
    ///
    /// TODO: Should we introduce a trait to explicitly mark the type as
    ///       user-copyable?
    pub unsafe fn read_uninit<T>(self, dst: &mut MaybeUninit<T>) -> Result<&mut T, ErrorCode> {
        // SAFETY: The caller must ensure that it is safe to copy.
        unsafe { self.read(dst.as_mut_ptr().cast(), size_of::<T>())? };
        Ok(unsafe { dst.assume_init_mut() })
    }

    /// Reads the user address into a kernel buffer.
    ///
    /// # Safety
    ///
    /// - `ptr` must be a non-null pointer.
    /// - The buffer must be at least `len` bytes long.
    pub unsafe fn read(self, ptr: *mut u8, len: usize) -> Result<(), ErrorCode> {
        if len != self.len() {
            return Err(ErrorCode::INVALID_ARG);
        }

        unsafe { usercopy_read(self.addr, ptr, len) }
    }
}
