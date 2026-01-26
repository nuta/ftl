use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr::NonNull;

use ftl_arrayvec::ArrayVec;
use ftl_bump_allocator::BumpAllocator;
use ftl_utils::alignment::is_aligned;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::arch::MIN_PAGE_SIZE;
use crate::spinlock::SpinLock;

/// The physical memory allocator.
pub static PAGE_ALLOCATOR: PageAllocator = PageAllocator::new();

/// The kernel heap allocator.
#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub struct PageAllocator {
    regions: SpinLock<ArrayVec<BumpAllocator, 8>>,
}

impl PageAllocator {
    const fn new() -> Self {
        Self {
            regions: SpinLock::new(ArrayVec::new()),
        }
    }

    pub fn add_region(&self, paddr: PAddr, size: usize) {
        if self
            .regions
            .lock()
            .try_push(BumpAllocator::new(paddr.as_usize(), size))
            .is_err()
        {
            println!("too many memory regions: {} ({})", paddr, size);
        }
    }

    /// Allocates a min-page-aligned memory block.
    ///
    /// `len` is the size in bytes to allocate, and must be a multiple of the
    /// minimum page size (typically 4096 bytes).
    pub fn alloc(&self, len: usize) -> Option<PAddr> {
        assert!(is_aligned(len, MIN_PAGE_SIZE));

        let mut regions = self.regions.lock();
        for region in regions.iter_mut() {
            if let Some(addr) = region.alloc(len, MIN_PAGE_SIZE) {
                return Some(PAddr::new(addr));
            }
        }

        None
    }
}

struct OomHandler;

impl talc::OomHandler for OomHandler {
    fn handle_oom(_talc: &mut talc::Talc<Self>, layout: Layout) -> Result<(), ()> {
        panic!("out of memory: {:?}", layout);
    }
}

struct GlobalAllocator {
    inner: SpinLock<talc::Talc<OomHandler>>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            inner: SpinLock::new(talc::Talc::new(OomHandler)),
        }
    }

    pub unsafe fn add_region(&self, vaddr: VAddr, size: usize) {
        if vaddr.as_usize().checked_add(size).is_none() {
            println!("too large kernel memory region: base={vaddr}, size={size}");
            return;
        }

        let ptr = vaddr.as_usize() as *mut u8;
        unsafe {
            let span = talc::Span::new(ptr, ptr.add(size));
            if self.inner.lock().claim(span).is_err() {
                println!("failed to claim kernel memory region: base={vaddr}, size={size}");
            }
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_assert!(layout.size() != 0);

        // SAFETY: The debug_assert ensures that the size is non-zero, which is
        // the precondition for malloc.
        unsafe {
            self.inner
                .lock()
                .malloc(layout)
                .expect("failed to allocate kernel memory")
                .as_ptr()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_assert!(layout.size() != 0);

        // SAFETY: ptr is allocated by talc, which returns a NonNull when
        // allocating memory and is guaranteed to be non-null.
        let nonnull = unsafe { NonNull::new_unchecked(ptr) };

        // SAFETY: ptr is allocated by this allocator.
        unsafe {
            self.inner.lock().free(nonnull, layout);
        }
    }
}

/// A wrapper struct to make a type page-aligned.
#[repr(align(4096))]
struct PageAligned<T>(T);

/// A temporary boot-time RAM area for the kernel's global allocator.
static mut EARLY_RAM: PageAligned<[u8; EARLY_RAM_SIZE]> = PageAligned([0; EARLY_RAM_SIZE]);
const EARLY_RAM_SIZE: usize = 128 * 1024; // 128 KB

pub fn init() {
    unsafe {
        let vaddr = VAddr::new(&raw mut EARLY_RAM as usize);
        GLOBAL_ALLOCATOR.add_region(vaddr, EARLY_RAM_SIZE);
    }
}
