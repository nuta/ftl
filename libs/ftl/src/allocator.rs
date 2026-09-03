use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::ptr;

use ftl_malloc::LinkedListAllocator;
use ftl_utils::spinlock::SpinLock;

const HEAP_SIZE: usize = 8 * 1024 * 1024;

#[repr(align(4096))]
struct Heap(UnsafeCell<[u8; HEAP_SIZE]>);

unsafe impl Sync for Heap {}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
static HEAP: Heap = Heap(UnsafeCell::new([0; HEAP_SIZE]));

struct GlobalAllocator {
    inner: SpinLock<Option<LinkedListAllocator>>,
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

        // Initialize the allocator if this is the first allocation.
        let allocator = inner.get_or_insert_with(|| {
            let mut allocator = LinkedListAllocator::new();
            let start = HEAP.0.get().cast::<u8>();
            // SAFETY: `HEAP` is statically allocated, and is exclusive to
            //         this allocator.
            unsafe {
                allocator.add_chunk(start, HEAP_SIZE);
            }
            allocator
        });

        allocator
            .malloc(layout.size(), layout.align())
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let mut inner = self.inner.lock();
        let allocator = inner.as_mut().unwrap();
        unsafe {
            allocator.free(ptr);
        }
    }
}
