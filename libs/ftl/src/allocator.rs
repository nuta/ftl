use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl_malloc::LinkedListAllocator;
use ftl_types::handle::HandleId;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::spinlock::SpinLock;

use crate::vmo::Vmo;
use crate::vmspace::VmSpace;

const CHUNK_SIZE: usize = 128 * 1024;
const HEAP_BASE_ADDR: usize = 0x6000_0000;

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

static HEAP_NEXT_ADDR: AtomicUsize = AtomicUsize::new(HEAP_BASE_ADDR);

struct GlobalAllocator {
    vmspace: VmSpace,
    inner: SpinLock<LinkedListAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            // SAFETY: The kernel loader installs the process vmspace as handle 2.
            vmspace: unsafe { VmSpace::from_handle(HandleId::new(2)) },
            inner: SpinLock::new(LinkedListAllocator::new()),
        }
    }

    fn request_chunk(&self) -> Result<(), ()> {
        // Get the address for the next chunk.
        let uaddr = HEAP_NEXT_ADDR.fetch_add(CHUNK_SIZE, Ordering::Relaxed);

        // Allocate a new VMO for the chunk.
        let vmo = match Vmo::create(CHUNK_SIZE) {
            Ok(vmo) => vmo,
            Err(e) => {
                warn!("failed to create VMO for heap chunk: {:?}", e);
                return Err(());
            }
        };

        // Map the chunk to the VM space.
        if let Err(e) = self
            .vmspace
            .map(&vmo, uaddr, PageAttrs::READ | PageAttrs::WRITE)
        {
            warn!("failed to map VMO for heap chunk: {:?}", e);
            return Err(());
        }

        // Add the chunk to the allocator.
        let mut allocator = self.inner.lock();
        // SAFETY: The mapped chunk is exclusive to this allocator.
        unsafe {
            allocator.add_chunk(uaddr as *mut u8, CHUNK_SIZE);
        }

        Ok(())
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Try allocating from existing chunks.
        let mut allocator = self.inner.lock();
        if let Some(ptr) = allocator.malloc(layout.size(), layout.align()) {
            return ptr;
        }

        // Out of memory. Request a new chunk.
        drop(allocator);
        if self.request_chunk().is_err() {
            return ptr::null_mut();
        }

        // Try allocating again from the new chunk.
        let mut allocator = self.inner.lock();
        allocator
            .malloc(layout.size(), layout.align())
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let mut allocator = self.inner.lock();
        unsafe {
            allocator.free(ptr);
        }
    }
}
