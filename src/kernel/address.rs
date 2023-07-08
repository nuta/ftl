use core::{
    fmt::{self, Debug},
    mem::size_of,
    num::NonZeroUsize,
};

use crate::arch;

/// A non-NULL kernel virtual address.
///
/// This is similar to [`core::ptr::NonNull<T>`]. Note that this type doesn't track the
/// lifetime of the referenced value: it's up to the user to ensure it's still valid
/// when dereferencing.
///
/// To represent NULL-able value, use `Option<VAddr>`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VAddr(NonZeroUsize);

impl VAddr {
    /// Creates a new `VAddr`.
    ///
    /// # Panics
    ///
    /// This function panics if `addr` is zero or not valid address.
    #[inline]
    pub const fn new(addr: usize) -> VAddr {
        assert!(addr != 0, "VAddr cannot be zero");
        assert!(arch::is_valid_vaddr(addr), "VAddr is not a valid address");

        // SAFETY: The assert above ensures that `addr` is not zero.
        VAddr(unsafe { NonZeroUsize::new_unchecked(addr) })
    }

    /// Creates a new `VAddr` from a `NonZeroUsize`.
    #[inline]
    pub const fn from_nonzero_usize(addr: NonZeroUsize) -> VAddr {
        VAddr(addr)
    }

    /// Returns `self + offset`.
    ///
    /// Note that this method doesn't mutate `self`: it returns a new `VAddr`.
    ///
    /// # Panics
    ///
    /// This function panics on overflow.
    #[inline]
    #[must_use]
    pub const fn offset(self, offset: usize) -> VAddr {
        VAddr::new(self.as_usize() + offset)
    }

    /// Returns the address.
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0.get()
    }

    /// Returns a pointer to the address.
    ///
    /// # Safety
    ///
    /// The caller must ensure the following:
    ///
    /// - The address is properly aligned.
    /// - The referenced value is live (i.e. not dangling).
    /// - The address is accessible (i.e. mapped in the page table).
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.as_usize() as *const T
    }

    /// Returns a mutable pointer to the address.
    ///
    /// # Safety
    ///
    /// The caller must ensure the following:
    ///
    /// - The address is properly aligned.
    /// - The address is live
    /// - The referenced value is live (i.e. not dangling).
    /// - The address is accessible (i.e. mapped in the page table).
    /// - It's safe to mutate: no concurrent access to the same address.
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.as_usize() as *mut T
    }

    /// Returns a reference to the value at the address, as arbitrary `&'a T`.
    ///
    /// # Safety
    ///
    /// The caller must ensure the following:
    ///
    /// - The address is properly aligned.
    /// - The address is accessible (i.e. mapped in the page table).
    /// - The reference value is valid for `'a`.
    /// - The reference value is initialized.
    #[inline]
    pub const unsafe fn as_ref<'a, T>(self) -> &'a T {
        &*self.as_ptr()
    }

    /// Returns a mutable reference to the value at the address, as arbitrary `&'a T`.
    ///
    /// # Safety
    ///
    /// The caller must ensure the following:
    ///
    /// - The address is properly aligned.
    /// - The address is accessible (i.e. mapped in the page table).
    /// - The reference value is valid for `'a`.
    /// - The reference value is initialized.
    /// - It's safe to mutate: no concurrent access to the same address.
    #[inline]
    pub const unsafe fn as_mut<'a, T>(self) -> &'a mut T {
        &mut *self.as_mut_ptr()
    }
}

impl Debug for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match size_of::<usize>() {
            4 => write!(f, "0x{:#08x}", self.as_usize()),
            8 => write!(f, "0x{:#016x}", self.as_usize()),
            _ => unreachable!(),
        }
    }
}

/// A non-NULL physical address.
///
/// To represent NULL-able value, use `Option<PAddr>`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PAddr(NonZeroUsize);

impl PAddr {
    /// Creates a new `PAddr`.
    ///
    /// # Panics
    ///
    /// This function panics if `addr` is zero.
    #[inline]
    pub const fn new(addr: usize) -> VAddr {
        assert!(addr != 0, "PAddr cannot be zero");

        // SAFETY: The assert above ensures that `addr` is not zero.
        PAddr(unsafe { NonZeroUsize::new_unchecked(addr) })
    }

    /// Creates a new `VAddr` from a `NonZeroUsize`.
    #[inline]
    pub const fn from_nonzero_usize(addr: NonZeroUsize) -> PAddr {
        PAddr(addr)
    }
}
