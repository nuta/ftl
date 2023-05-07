//! A bump memory allocator.
//!
//! Unlike typical allocators, this allocator does not support freeing memory.
//! Instead, it only supports allocating memory. This makes it extremely fast and
//! simple.
//!
//! Typically, this allocator is used for allocating memory in initialization
//! phase such that the allocated memory is never freed.
//!
//! # Examples
//!
//! ```
//! use bump_allocator::BumpAllocator;
//!
//! let mut allocator = BumpAllocator::new();
//! allocator.add_region(0x20000 /* base */, 0x1000 /* len */);
//! let addr1 = allocator.alloc(0x100 /* size */, 0x100 /* align */);
//! ```
#![no_std]
#![cfg_attr(test, feature(test))]

use core::num::NonZeroUsize;

#[cfg(test)]
extern crate test;

fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

/// A bump memory allocator.
pub struct BumpAllocator {
    top: usize,
    bottom: usize,
}

impl BumpAllocator {
    // Creates a new bump allocator. Initially, the allocator has no memory
    // region. Call `add_region` to add a memory region.
    pub const fn new() -> BumpAllocator {
        BumpAllocator { bottom: 0, top: 0 }
    }

    // Gives a meory region `[base, base + len)` to the allocator.
    // `base` must be non-zero.
    pub fn add_region(&mut self, base: usize, len: usize) {
        debug_assert!(self.bottom == 0, "only one region is supported");
        debug_assert!(base > 0);

        self.bottom = base;
        self.top = base + len;
    }

    /// Allocates `size` bytes of memory with the given `align` bytes alignment.
    /// Returns the beginning address of the allocated memory if successful.
    pub fn allocate(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<NonZeroUsize> {
        let aligned_size = align_up(size, align);
        let new_top = self.top.checked_sub(aligned_size)?;
        if new_top < self.bottom {
            return None;
        }

        self.top = new_top;

        // SAFETY: `self.top` is checked to be larger than `self.bottom`.
        unsafe { Some(NonZeroUsize::new_unchecked(self.top)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroUsize;

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn test_bump_allocator() {
        let mut allocator = BumpAllocator::new();
        allocator.add_region(0x20000, 0x4000);
        assert_eq!(allocator.allocate(0x1000, 0x1000), Some(nonzero(0x23000)));
        assert_eq!(allocator.allocate(0x1000, 0x1000), Some(nonzero(0x22000)));
        assert_eq!(allocator.allocate(0xf00, 0x1000), Some(nonzero(0x21000)));
        assert_eq!(allocator.allocate(0x1000, 0x1000), Some(nonzero(0x20000)));
        assert_eq!(allocator.allocate(0x1000, 0x1000), None);
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_bump_allocator(b: &mut Bencher) {
        let mut allocator = BumpAllocator::new();
        allocator.add_region(0x10000, 0x10000000);
        b.iter(|| test::black_box(allocator.allocate(0x1000, 0x1000)));
    }
}
