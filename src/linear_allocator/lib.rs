//! A linear memory allocator.
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
//! use linear_allocator::LinearAllocator;
//!
//! let mut allocator = LinearAllocator::new(0x20000 /* base */, 0x1000 /* len */);
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

/// A linear memory allocator.
pub struct LinearAllocator {
    top: usize,
    bottom: usize,
}

impl LinearAllocator {
    // Creates a new linear allocator. The allocator will allocate memory in the
    // range `[base, base + len)`. `base` must be non-zero.
    pub fn new(base: usize, len: usize) -> Self {
        debug_assert!(base > 0);
        Self {
            bottom: base,
            top: base + len,
        }
    }

    /// Allocates `size` bytes of memory with the given `align` bytes alignment.
    /// Returns the beginning address of the allocated memory if successful.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<NonZeroUsize> {
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
    use core::num::NonZeroUsize;

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn test_linear_allocator() {
        let mut allocator = super::LinearAllocator::new(0x20000, 0x4000);
        assert_eq!(allocator.alloc(0x1000, 0x1000), Some(nonzero(0x23000)));
        assert_eq!(allocator.alloc(0x1000, 0x1000), Some(nonzero(0x22000)));
        assert_eq!(allocator.alloc(0xf00, 0x1000), Some(nonzero(0x21000)));
        assert_eq!(allocator.alloc(0x1000, 0x1000), Some(nonzero(0x20000)));
        assert_eq!(allocator.alloc(0x1000, 0x1000), None);
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_linear_allocator(b: &mut Bencher) {
        let mut allocator = LinearAllocator::new(0x10000, 0x10000000);
        b.iter(|| test::black_box(allocator.alloc(0x1000, 0x1000)));
    }
}
