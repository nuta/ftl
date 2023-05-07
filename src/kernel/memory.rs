use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    ptr::NonNull,
};

use crate::lock::GiantLock;

use bump_allocator::BumpAllocator;
use linked_list_allocator::Heap;

/// The size of the malloc heap in bytes. To be used in `core::alloc` objects.
const MALLOC_SIZE: usize = 16 * 1024 * 1024;

pub static PAGE_ALLOCATOR: GiantLock<BumpAllocator> =
    GiantLock::new(BumpAllocator::new());

struct HeapAllocator(GiantLock<Heap>);

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator =
    HeapAllocator(GiantLock::new(Heap::empty()));

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .get_mut()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .get_mut()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

extern "C" {
    static __boot_heap: u8;
    static __boot_heap_end: u8;
}

pub fn init() {
    unsafe {
        let heap_start = &__boot_heap as *const u8 as usize;
        let heap_end = &__boot_heap_end as *const u8 as usize;
        PAGE_ALLOCATOR
            .get_mut()
            .add_region(heap_start, heap_end - heap_start);

        let malloc_start = PAGE_ALLOCATOR
            .get_mut()
            .allocate(MALLOC_SIZE, size_of::<usize>())
            .map(|addr| addr.get() as *mut u8)
            .expect("failed to allocate malloc heap");
        HEAP_ALLOCATOR.0.get_mut().init(malloc_start, MALLOC_SIZE);
    }
}
