use core::alloc::GlobalAlloc;
use core::alloc::Layout;

use bump_allocator::BumpAllocator;

use crate::boot::BootInfo;
use crate::boot::FreeMem;
use crate::print::ByteSize;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub struct GlobalAllocator {
    inner: spin::Mutex<BumpAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> GlobalAllocator {
        let allocator = BumpAllocator::new();

        GlobalAllocator {
            inner: spin::Mutex::new(allocator),
        }
    }

    pub fn add_region(&self, heap: *mut u8, heap_len: usize) {
        self.inner.lock().add_region(heap as usize, heap_len);
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
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

pub fn alloc_pages(num_pages: usize) -> Option<usize> {
    GLOBAL_ALLOCATOR
        .inner
        .lock()
        .allocate(num_pages * 4096, 4096)
        .map(|addr| addr.get())
}

pub fn init(bootinfo: &BootInfo) {
    for entry in bootinfo.free_mems.iter() {
        match *entry {
            FreeMem { start, size } => {
                println!(
                    "free memory: 0x{:016x} - 0x{:016x} ({})",
                    start,
                    start + size,
                    ByteSize::new(size)
                );

                GLOBAL_ALLOCATOR.add_region(start as *mut u8, size);
            }
        }
    }
}
