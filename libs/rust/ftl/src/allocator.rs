use core::alloc::GlobalAlloc;
use core::alloc::Layout;

use ftl_malloc::LinkedListAllocator;
use ftl_utils::spinlock::SpinLock;

const MALLOC_CHUNK_SIZE: usize = 128 * 1024; // 128 KB

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

fn sys_malloc(size: usize) -> Result<*mut u8, ()> {
    todo!()
}

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

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        if let Some(ptr) = inner.malloc(layout.size(), layout.align()) {
            return ptr;
        }

        // Allocate more memory from the kernel.
        let ptr = match sys_malloc(MALLOC_CHUNK_SIZE) {
            Ok(ptr) => ptr,
            Err(error) => {
                panic!("failed to malloc from kernel: {:?}", error);
            }
        };

        // SAFETY: malloc is guaranteed to return a valid pointer
        //         when it returns Ok(ptr).
        unsafe {
            inner.add_chunk(ptr, MALLOC_CHUNK_SIZE);
        }

        // Try to allocate from the new chunk.
        if let Some(ptr) = inner.malloc(layout.size(), layout.align()) {
            return ptr;
        }

        panic!(
            "failed to malloc from new chunk: size={}, align={}",
            layout.size(),
            layout.align()
        );
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            self.inner.lock().free(ptr);
        }
    }
}
