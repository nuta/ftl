use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr;

use ftl_malloc::LinkedListAllocator;
use ftl_utils::spinlock::SpinLock;

#[cfg_attr(target_os = "none", global_allocator)]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

struct GlobalAllocator {
    inner: SpinLock<LinkedListAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            inner: SpinLock::new(LinkedListAllocator::new()),
        }
    }
}

pub unsafe fn init(ptr: *mut u8, size: usize) {
    unsafe {
        GLOBAL_ALLOCATOR.inner.lock().add_chunk(ptr, size);
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner
            .lock()
            .malloc(layout.size(), layout.align())
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            self.inner.lock().free(ptr);
        }
    }
}
