use core::alloc::{GlobalAlloc, Layout};

use bump_allocator::BumpAllocator;

use crate::giant_lock::GiantLock;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub struct GlobalAllocator {
    inner: GiantLock<BumpAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> GlobalAllocator {
        let allocator = BumpAllocator::new();

        GlobalAllocator {
            inner: GiantLock::new(allocator),
        }
    }

    pub fn add_region(&self, heap: *mut u8, heap_len: usize) {
        self.inner.borrow_mut().add_region(heap as usize, heap_len);
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self
            .inner
            .borrow_mut()
            .allocate(layout.size(), layout.align())
            .map(|addr| addr.get() as *mut u8)
            .expect("failed to allocate memory");

        ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        /* Do nothing: this is well-known limitation of bump allocator! */
    }
}
