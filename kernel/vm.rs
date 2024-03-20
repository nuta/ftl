use core::num::NonZeroUsize;
use core::ops::Deref;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;

/// A virtual address *always* accessible from the kernel, i.e., it's always
/// mapped to the kernel's address space.
///
/// # Avoid using this type if possible!
///
/// This is a low-level type which can easily cause use-after-free bugs because
/// it doesn't track the lifetime of the underlying memory. Instead, prefer
/// [`alloc`] crate's types like [`alloc::boxed::Box`] or [`alloc::sync::Arc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KVAddr(VAddr);

impl KVAddr {
    /// Creates a new `KVAddr` from the given virtual address.
    pub const fn from_nonzero(addr: NonZeroUsize) -> KVAddr {
        KVAddr(VAddr::from_nonzero(addr))
    }

    /// Returns the virtual address.
    pub fn vaddr(&self) -> VAddr {
        self.0
    }
}

impl Deref for KVAddr {
    type Target = VAddr;

    fn deref(&self) -> &VAddr {
        &self.0
    }
}

/// Methods that `crate::arch` should implement for `KVAddr`.
pub trait KVAddrArchExt {
    /// Returns the physical address of the virtual address.
    ///
    /// This should never fail, because `KVAddr`'s precondition is that it's
    /// always mapped to the kernel's address space.
    fn paddr(&self) -> PAddr;
}
