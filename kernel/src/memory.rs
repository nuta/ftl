use core::alloc::GlobalAlloc;
use core::alloc::Layout;

use ftl_malloc::LinkedListAllocator;
use ftl_utils::formatter::ByteSize;

use crate::address::PAddr;
use crate::boot::BootInfo;
use crate::boot::FreeRam;
use crate::spinlock::SpinLock;

/// A wrapper struct to make a type page-aligned.
#[repr(align(4096))]
struct PageAligned<T>(T);

/// A temporary boot-time RAM area for the kernel's global allocator.
static mut EARLY_RAM: PageAligned<[u8; EARLY_RAM_SIZE]> = PageAligned([0; EARLY_RAM_SIZE]);
const EARLY_RAM_SIZE: usize = 128 * 1024; // 128 KB

#[global_allocator]
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

    pub fn add_region(&self, ptr: *mut u8, size: usize) {
        unsafe {
            self.inner.lock().add_chunk(ptr, size);
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.inner.lock().malloc(layout.size(), layout.align()) {
            Some(ptr) => ptr,
            None => {
                panic!(
                    "failed to malloc: size={}, align={}",
                    layout.size(),
                    layout.align()
                );
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            self.inner.lock().free(ptr);
        }
    }
}

pub fn init(bootinfo: &BootInfo) {
    unsafe {
        let ptr = &raw mut EARLY_RAM.0 as *mut _ as *mut u8;
        GLOBAL_ALLOCATOR.add_region(ptr, EARLY_RAM_SIZE);
    }

    for FreeRam { addr, size } in &bootinfo.free_rams {
        let end = PAddr::new(addr.as_usize() + size);
        // TODO: exclude kernel memory range

        // QEMU does not exclude module regions from the free RAM regions. Exclude
        // them manually so that the kernel won't try to allocate from them.

        trace!("RAM: {addr} - {end} ({})", ByteSize(*size));
    }
}
