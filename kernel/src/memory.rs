use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::ptr::NonNull;

use ftl_arrayvec::ArrayVec;
use ftl_bump_allocator::BumpAllocator;
use ftl_types::error::ErrorCode;
use ftl_utils::alignment::is_aligned;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::boot::BootInfo;
use crate::boot::FreeRam;
use crate::isolation::INKERNEL_ISOLATION;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::thread::Thread;

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

    pub fn add_regions(&self, free_rams: &ArrayVec<FreeRam, 8>) {
        let mut regions = self.regions.lock();

        for free_ram in free_rams {
            let Some(end) = free_ram.base.as_usize().checked_add(free_ram.size) else {
                println!("the size of the memory region overflows: {free_ram:?}");
                return;
            };

            let allocator = BumpAllocator::new(free_ram.base.as_usize(), end);
            if regions.try_push(allocator).is_err() {
                println!("failed to add memory region: {free_ram:?}");
                return;
            }
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

pub fn sys_dmabuf_alloc(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<usize, ErrorCode> {
    let size = a0;
    let vaddr_ptr = UserSlice::new(UserPtr::new(a1), size_of::<usize>())?;
    let paddr_ptr = UserSlice::new(UserPtr::new(a2), size_of::<usize>())?;

    if !is_aligned(size, MIN_PAGE_SIZE) {
        return Err(ErrorCode::InvalidArgument);
    }

    // Allocate physical memory.
    //
    // TODO: Support constraints like alignment.
    let Some(paddr) = PAGE_ALLOCATOR.alloc(size) else {
        return Err(ErrorCode::OutOfMemory);
    };

    // Map the allocated physical memory to the process's address space.
    let isolation = current.process().isolation();
    let vaddr = if SharedRef::ptr_eq(isolation, &INKERNEL_ISOLATION) {
        arch::paddr2vaddr(paddr)
    } else {
        return Err(ErrorCode::Unsupported);
    };

    crate::isolation::write(isolation, vaddr_ptr, 0, vaddr.as_usize())?;
    crate::isolation::write(isolation, paddr_ptr, 0, paddr.as_usize())?;
    Ok(0)
}

/// A wrapper struct to make a type page-aligned.
#[repr(align(4096))]
struct PageAligned<T>(T);

/// A temporary boot-time RAM area for the kernel's global allocator.
static mut EARLY_RAM: PageAligned<[u8; EARLY_RAM_SIZE]> = PageAligned([0; EARLY_RAM_SIZE]);
const EARLY_RAM_SIZE: usize = 128 * 1024; // 128 KB

pub fn init(bootinfo: &BootInfo) {
    PAGE_ALLOCATOR.add_regions(&bootinfo.free_rams);

    unsafe {
        let vaddr = VAddr::new(&raw mut EARLY_RAM as usize);
        GLOBAL_ALLOCATOR.add_region(vaddr, EARLY_RAM_SIZE);
    }
}
