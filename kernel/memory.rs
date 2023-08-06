use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    ptr::{addr_of, NonNull},
};

use crate::{
    address::{PAddr, VAddr},
    arch::PAGE_SIZE,
    giant_lock::{GiantLock, GiantLockGuard},
    memory_pool::{self, memory_pool_mut, MemoryPool},
};

use bump_allocator::BumpAllocator;
use linked_list_allocator::Heap;

/// The size of the malloc heap in bytes. To be used in `core::alloc` objects.
const MALLOC_SIZE: usize = 16 * 1024 * 1024;

static PAGE_ALLOCATOR: GiantLock<BumpAllocator> =
    GiantLock::new(BumpAllocator::new());

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator =
    HeapAllocator(GiantLock::new(Heap::empty()));

/// A heap allocator to enable alloc crate in the kernel. Intended to be used
/// for debugging purposes only: we prefer not to do dynamic memory allocation
/// in the kernel and instead userland specifies kernel memory space as needed.
struct HeapAllocator(GiantLock<Heap>);

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

/// Allocates memory pages for boot-time use. `size` is the size in bytes.
///
/// Returns the address of the beginning of the range or `None` if the
/// allocation failed.
///
/// ## Guarantees
///
/// - All allocated pages are contiguous, in both virtual and physical
///   address space.
/// - The returned address is aligned to `PAGE_SIZE`.
/// - The returned address is accessible by the kernel (i.e. it's mapped in
///   the page table).
///
/// ## Limitations
///
/// - You can't free the allocated memory. It's intended to be used for
///   boot-time initialization.
#[track_caller]
pub fn allocate_pages(size: usize) -> Option<VAddr> {
    PAGE_ALLOCATOR
        .borrow_mut()
        .allocate(size, PAGE_SIZE)
        .map(|addr| VAddr::from_nonzero_usize(addr))
}

pub fn allocate_and_initialize<F, T>(len: usize, initializer: F) -> T
where
    F: FnOnce(&mut GiantLockGuard<'_, MemoryPool>, VAddr) -> T,
{
    let vaddr = allocate_pages(len).expect("failed to allocate pages");
    let mut pool =
        memory_pool_mut(vaddr).expect("no corresponding memory pool");
    initializer(&mut pool, vaddr)
}

/// Allocates all remaining memory pages.
///
/// Returns the address of the beginning of the range and the size of the range
/// in bytes, or `None` if the allocation failed.
///
/// This function is intended to be used for giving all remaining memory that
/// the kernel doesn't need to userland.
///
/// The allocated size may NOT be aligned to `PAGE_SIZE`.
///
/// ## Guarantees
///
/// - All allocated pages are contiguous, in both virtual and physical
///   address space.
/// - The returned address is aligned to `PAGE_SIZE`.
///
/// ## Limitations
///
/// - You can't free the allocated memory. It's intended to be used for
///   boot-time initialization.
pub fn allocate_all_pages() -> Option<(PAddr, usize)> {
    PAGE_ALLOCATOR
        .borrow_mut()
        .allocate_all(PAGE_SIZE)
        .map(|(addr, size)| (PAddr::from_nonzero_usize(addr), size))
}

/// Initializes the memory subsystem.
pub fn init() {
    extern "C" {
        static __free_ram: u8;
        static __free_ram_end: u8;
    }

    unsafe {
        let free_ram_start = addr_of!(__free_ram) as usize;
        let free_ram_end = addr_of!(__free_ram_end) as usize;
        println!("free ram start: {:016x}", free_ram_start);
        println!("free ram end:   {:016x}", free_ram_end);

        memory_pool::init(VAddr::new(free_ram_start), free_ram_end - free_ram_start);
        // TODO: What if the heap spans multiple memory pools?
        let pool = memory_pool_mut(VAddr::new(free_ram_start)).unwrap();

        println!("memory region: {:016x} - {:016x}", pool.base().as_usize(), pool.base().as_usize() + pool.len());

        // Initialize the boot-time page allocator.
        PAGE_ALLOCATOR
            .borrow_mut()
            .add_region(pool.base().as_usize(), pool.len());

        // Allocate memory for the malloc heap.
        let malloc_start = PAGE_ALLOCATOR
        .borrow_mut()
        .allocate(MALLOC_SIZE, size_of::<usize>())
        .map(|addr| addr.get() as *mut u8)
        .expect("failed to allocate malloc heap");

    // Initialize the malloc heap.
        HEAP_ALLOCATOR
        .0
        .borrow_mut()
        .init(malloc_start, MALLOC_SIZE);
    }
}
