#![no_std]

use ftl_utils::alignment::align_down;

/// A bump memory allocator.
///
/// # Example
///
/// ```
/// use ftl_bump_allocator::BumpAllocator;
///
/// let mut allocator = BumpAllocator::new(0x1000, 0x2000);
/// let addr = allocator.alloc(0x100, 0x100);
/// assert_eq!(addr, Some(0x1f00));
/// ```
pub struct BumpAllocator {
    /// The start address of the memory region.
    start: usize,
    /// The next address to allocate from. Grows downwards from the end of the
    /// memory region.
    next: usize,
}

impl BumpAllocator {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, next: end }
    }

    /// Allocates a block of memory.
    ///
    /// Returns the address of the allocated block, or `None` if it does not fit.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        let addr = self.next.checked_sub(size)?;
        let aligned_addr = align_down(addr, align);

        if aligned_addr < self.start {
            return None;
        }

        self.next = aligned_addr;
        Some(aligned_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        let mut allocator = BumpAllocator::new(0, 0x2000);
        assert_eq!(allocator.alloc(0x5, 0x10), Some(0x1ff0));
        assert_eq!(allocator.alloc(0x5, 0x10), Some(0x1fe0));
        assert_eq!(allocator.alloc(0x1000, 0x1000), Some(0));
        assert_eq!(allocator.alloc(0x1000, 0x1000), None);
    }

    #[test]
    fn test_zero() {
        let mut allocator = BumpAllocator::new(0, 0);
        assert_eq!(allocator.alloc(0x1000, 0x1000), None);
    }
}
