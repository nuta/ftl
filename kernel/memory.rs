use core::alloc::GlobalAlloc;
use core::alloc::Layout;

use ftl_bump_allocator::BumpAllocator;

use crate::boot::BootInfo;
use crate::boot::FreeMem;
use crate::spinlock::SpinLock;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub struct GlobalAllocator {
    inner: SpinLock<BumpAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> GlobalAllocator {
        let allocator = BumpAllocator::new();

        GlobalAllocator {
            inner: SpinLock::new(allocator),
        }
    }

    pub fn add_region(&self, heap: *mut u8, heap_len: usize) {
        self.inner.lock().add_region(heap as usize, heap_len);
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_assert!(layout.size() > 0, "size must be greater than 0");

        let ptr = self
            .inner
            .lock()
            .allocate(layout.size(), layout.align())
            .map(|addr| addr.get() as *mut u8)
            .expect("failed to allocate memory");

        // println!("alloc: {:p} {:?}", ptr, layout);
        ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        /* Do nothing: this is well-known limitation of bump allocator! */
    }
}

pub fn init(bootinfo: &BootInfo) {
    for entry in bootinfo.free_mems.iter() {
        match *entry {
            FreeMem { start, size } => {
                println!(
                    "free memory: 0x{:016x} - 0x{:016x} ({} MiB)",
                    start,
                    start + size,
                    size / 1024 / 1024
                );

                GLOBAL_ALLOCATOR.add_region(start as *mut u8, size);
            }
        }
    }
}
