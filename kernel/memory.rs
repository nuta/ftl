use core::alloc::GlobalAlloc;
use core::alloc::Layout;

use ftl_bump_allocator::BumpAllocator;
use ftl_types::error::FtlError;
use ftl_utils::byte_size::ByteSize;

use crate::boot::BootInfo;
use crate::boot::FreeMem;
use crate::spinlock::SpinLock;
use crate::vm::KVAddr;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

/// The default in-kernel memory allocator.
///
/// Allocated memory are always accessible from the kernel's address space,
/// that is, memory pages added to this allocator must not be swapped out,
/// or something like that.
pub struct GlobalAllocator {
    inner: SpinLock<BumpAllocator>,
}

impl GlobalAllocator {
    /// Creates a new global allocator.
    ///
    /// The allocator is initially empty. Memory regions must be added
    /// by calling [`GlobalAllocator::add_region`] method.
    pub const fn new() -> GlobalAllocator {
        let allocator = BumpAllocator::new();

        GlobalAllocator {
            inner: SpinLock::new(allocator),
        }
    }

    /// Adds a new memory region to the allocator.
    ///
    /// The memory region must be always mapped to the kernel's address space.
    pub fn add_region(&self, heap: *mut u8, heap_len: usize) {
        self.inner.lock().add_region(heap as usize, heap_len);
    }

    /// Allocates memory.
    pub fn alloc_as_kvaddr(&self, layout: Layout) -> Result<KVAddr, FtlError> {
        self.inner
            .lock()
            .allocate(layout.size(), layout.align())
            .map(|addr| KVAddr::from_nonzero(addr))
            .ok_or(FtlError::OutOfMemory)
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_assert!(layout.size() > 0, "size must be greater than 0");

        self.alloc_as_kvaddr(layout)
            .expect("failed to allocate memory")
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        /* We can't deallocate. This is well-known limitation of bump allocator! */
    }
}

/// Initializes the memory subsystem.
///
/// After this function is called, the global allocator (e.g. `Box`, `Vec`, etc.)
/// becomes available.
pub fn init(bootinfo: &BootInfo) {
    for FreeMem { start, size } in bootinfo.free_mems.iter() {
        println!(
            "free memory: 0x{:016x} - 0x{:016x} ({})",
            start,
            start + size,
            ByteSize(*size)
        );

        GLOBAL_ALLOCATOR.add_region(*start as *mut u8, *size);
    }
}
