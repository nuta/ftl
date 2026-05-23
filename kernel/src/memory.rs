use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cmp::min;
use core::ops::Range;

use ftl_arrayvec::ArrayVec;
use ftl_bump_allocator::BumpAllocator;
use ftl_malloc::LinkedListAllocator;
use ftl_utils::alignment::is_aligned;
use ftl_utils::formatter::ByteSize;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::boot::BootInfo;
use crate::boot::FreeRam;
use crate::boot::NUM_MODULES_MAX;
use crate::spinlock::SpinLock;

const MALLOC_CHUNK_SIZE: usize = 128 * 1024; // 128 KB

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
        let mut inner = self.inner.lock();
        if let Some(ptr) = inner.malloc(layout.size(), layout.align()) {
            return ptr;
        }

        // The global allocator is out of memory. Try to allocate more from the
        // page allocator.
        let Some(paddr) = PAGE_ALLOCATOR.alloc(MALLOC_CHUNK_SIZE, PageType::Dirty) else {
            panic!(
                "out of memory: size={}, align={}",
                layout.size(),
                layout.align()
            );
        };

        let ptr = arch::paddr2vaddr(paddr).as_mut_ptr();
        // SAFETY: The page allocator returns a valid pointer.
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

/// The physical memory allocator.
pub static PAGE_ALLOCATOR: PageAllocator = PageAllocator::new();

/// The type of pages to allocate.
pub enum PageType {
    /// The pages don't need to be zeroed. The caller is responsible for
    /// initializing the memory.
    Dirty,
    /// The pages need to be zeroed.
    Zeroed,
}

pub struct PageAllocator {
    regions: SpinLock<ArrayVec<BumpAllocator, 8>>,
}

impl PageAllocator {
    const fn new() -> Self {
        Self {
            regions: SpinLock::new(ArrayVec::new()),
        }
    }

    pub fn add_region(&self, start: PAddr, end: PAddr) {
        let mut regions = self.regions.lock();

        let allocator = BumpAllocator::new(start.as_usize(), end.as_usize());
        if regions.try_push(allocator).is_err() {
            trace!("too many free RAM regions");
            return;
        }
    }

    /// Allocates a min-page-aligned memory block.
    ///
    /// `len` is the size in bytes to allocate, and must be a multiple of the
    /// minimum page size (typically 4096 bytes).
    pub fn alloc(&self, len: usize, page_type: PageType) -> Option<PAddr> {
        debug_assert!(len > 0);
        debug_assert!(is_aligned(len, MIN_PAGE_SIZE));

        let mut regions = self.regions.lock();
        for region in regions.iter_mut() {
            if let Some(addr) = region.alloc(len, MIN_PAGE_SIZE) {
                let paddr = PAddr::new(addr);

                match page_type {
                    PageType::Dirty => {
                        // Do nothing.
                    }
                    PageType::Zeroed => {
                        let vaddr = arch::paddr2vaddr(paddr);
                        let ptr = vaddr.as_usize() as *mut u8;
                        unsafe {
                            core::ptr::write_bytes(ptr, 0, len);
                        }
                    }
                }

                return Some(paddr);
            }
        }

        None
    }
}

unsafe extern "C" {
    static __kernel_memory: u8;
    static __kernel_memory_end: u8;
}

fn kernel_reserved_range() -> Range<PAddr> {
    let start = VAddr::new(&raw const __kernel_memory as usize);
    let end = VAddr::new(&raw const __kernel_memory_end as usize);
    let start_paddr = arch::vaddr2paddr(start);
    let end_paddr = arch::vaddr2paddr(end);
    start_paddr..end_paddr
}

/// Calls `f` for each unused region between `addr` and `end`, excluding
/// the reserved regions and memory outside the direct map.
///
/// The `reserved_regions` must be sorted by the start address.
fn visit_unused_regions<F>(addr: PAddr, end: PAddr, reserved_regions: &[Range<PAddr>], mut f: F)
where
    F: FnMut(PAddr, PAddr),
{
    let end = min(end, arch::DIRECT_MAP_END);
    let mut cursor = addr;
    for reserved in reserved_regions {
        if cursor >= end {
            // The cursor is past the end of the RAM region.
            return;
        }

        if reserved.end <= cursor {
            // The reserved region is before the cursor. Keep checking the
            // following regions.
            continue;
        }

        if reserved.start >= end {
            // The reserved region is after the end of the RAM region and
            // following regions won't overlap. Stop here.
            break;
        }

        if cursor < reserved.start {
            // The cursor is before the start of the reserved region. Use the
            // gap as a free region.
            f(cursor, reserved.start);
        }

        cursor = reserved.end;
    }

    // The region after all reserved regions.
    if cursor < end {
        f(cursor, end);
    }
}

pub fn init(bootinfo: &BootInfo) {
    // Collect the reserved regions that we can't allocate from.
    let mut reserved_regions = ArrayVec::<Range<PAddr>, { NUM_MODULES_MAX + 1 }>::new();
    reserved_regions.try_push(kernel_reserved_range()).unwrap();
    for module in &bootinfo.modules {
        reserved_regions
            .try_push(module.start..module.end)
            .expect("too many reserved regions");
    }
    reserved_regions
        .as_slice_mut()
        .sort_unstable_by_key(|range| range.start);

    // Visit the free RAM regions and add them to the page allocator.
    for FreeRam { addr, size } in &bootinfo.free_rams {
        let Some(end) = addr.as_usize().checked_add(*size).map(PAddr::new) else {
            trace!("free RAM region overflows: {addr} + {}", ByteSize(*size));
            continue;
        };

        // QEMU does not exclude module regions from the free RAM regions. Exclude
        // them manually so that the kernel won't try to allocate from them.
        visit_unused_regions(*addr, end, reserved_regions.as_slice(), |addr, end| {
            let size = end.as_usize() - addr.as_usize();
            trace!("RAM: {addr} - {end} ({})", ByteSize(size));
            PAGE_ALLOCATOR.add_region(addr, end);
        });
    }
}
