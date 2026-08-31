use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr;

use ftl_bump_allocator::BumpAllocator;
use ftl_utils::spinlock::SpinLock;

const HEAP_SIZE: usize = 8 * 1024 * 1024;

#[repr(align(4096))]
struct Heap(UnsafeCell<[u8; HEAP_SIZE]>);

unsafe impl Sync for Heap {}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
static HEAP: Heap = Heap(UnsafeCell::new([0; HEAP_SIZE]));

struct GlobalAllocator {
    inner: SpinLock<Option<BumpAllocator>>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            inner: SpinLock::new(None),
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        let allocator = inner.get_or_insert_with(|| {
            let start = HEAP.0.get().cast::<u8>() as usize;
            BumpAllocator::new(start, start + HEAP_SIZE)
        });

        allocator
            .alloc(layout.size(), layout.align())
            .map_or(ptr::null_mut(), |addr| addr as *mut u8)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
