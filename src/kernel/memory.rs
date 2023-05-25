use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    num::NonZeroUsize,
    ptr::{addr_of, NonNull},
};

use crate::{arch::PAGE_SIZE, giant_lock::GiantLock};

use bump_allocator::BumpAllocator;
use linked_list_allocator::Heap;

/// The size of the malloc heap in bytes. To be used in `core::alloc` objects.
const MALLOC_SIZE: usize = 16 * 1024 * 1024;

static PAGE_ALLOCATOR: GiantLock<BumpAllocator> =
    GiantLock::new(BumpAllocator::new());

struct HeapAllocator(GiantLock<Heap>);

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator =
    HeapAllocator(GiantLock::new(Heap::empty()));

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .borrow_mut()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .borrow_mut()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

#[track_caller]
pub fn allocate_pages(size: usize) -> Option<NonZeroUsize> {
    PAGE_ALLOCATOR.borrow_mut().allocate(size, PAGE_SIZE)
}

pub fn allocate_all_pages() -> Option<(NonZeroUsize, usize)> {
    PAGE_ALLOCATOR.borrow_mut().allocate_all(PAGE_SIZE)
}

pub fn init() {
    extern "C" {
        static __boot_heap: u8;
        static __boot_heap_end: u8;
    }

    unsafe {
        let heap_start = addr_of!(__boot_heap) as usize;
        let heap_end = addr_of!(__boot_heap_end) as usize;

        // Initialize the boot-time page allocator.
        PAGE_ALLOCATOR
            .borrow_mut()
            .add_region(heap_start, heap_end - heap_start);

        // Initialize the malloc heap.
        let malloc_start = PAGE_ALLOCATOR
            .borrow_mut()
            .allocate(MALLOC_SIZE, size_of::<usize>())
            .map(|addr| addr.get() as *mut u8)
            .expect("failed to allocate malloc heap");
        HEAP_ALLOCATOR
            .0
            .borrow_mut()
            .init(malloc_start, MALLOC_SIZE);
    }
}
