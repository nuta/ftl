use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cmp::min;
use core::ops::Range;
use core::ptr::null_mut;

use ftl_bitmap_allocator::BitmapAllocator;
use ftl_malloc::LinkedListAllocator;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::alignment::is_aligned;
use ftl_utils::formatter::ByteSize;
use ftl_utils::spinlock::SpinLock;

use crate::address::PAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::boot::BootInfo;
use crate::boot::FreeRam;

const MALLOC_CHUNK_SIZE: usize = 512 * 1024; // 512 KB

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
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        if let Some(ptr) = inner.malloc(layout.size(), layout.align()) {
            return ptr;
        }

        if layout.size() > MALLOC_CHUNK_SIZE - ftl_malloc::HEADER_SIZE {
            // It is too large to allocate.
            warn!(
                "failed to malloc: size={}, align={}",
                layout.size(),
                layout.align()
            );
            return null_mut();
        }

        // The global allocator is out of memory. Try to allocate more from the
        // page allocator.
        let Some(paddr) = PAGE_ALLOCATOR.alloc(MALLOC_CHUNK_SIZE, PageType::Dirty) else {
            return null_mut();
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

        trace!(
            "failed to malloc from new chunk: size={}, align={}",
            layout.size(),
            layout.align()
        );
        return null_mut();
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
    inner: BitmapAllocator,
}

impl PageAllocator {
    const fn new() -> Self {
        Self {
            inner: BitmapAllocator::new(MIN_PAGE_SIZE),
        }
    }

    pub fn add_region(&self, start: PAddr, end: PAddr) {
        let start = align_up(start.as_usize(), MIN_PAGE_SIZE);
        let end = align_down(end.as_usize(), MIN_PAGE_SIZE);
        let len = end.saturating_sub(start);
        if len == 0 {
            warn!("free RAM region is empty: {start:x}");
            return;
        }

        let paddr = PAddr::new(start);
        let ptr = arch::paddr2vaddr(paddr).as_mut_ptr();
        if let Err(err) = unsafe { self.inner.add_chunk(ptr, start, len) } {
            trace!("failed to add free RAM region: {err:?}");
        }
    }

    /// Allocates a min-page-aligned memory block.
    ///
    /// `len` is the size in bytes to allocate, and must be a multiple of the
    /// minimum page size (typically 4096 bytes).
    pub fn alloc(&self, len: usize, page_type: PageType) -> Option<PAddr> {
        if len == 0 {
            trace!("tried to allocate 0 bytes");
            return None;
        }

        if !is_aligned(len, MIN_PAGE_SIZE) {
            trace!("tried to allocate unaligned size: {len}");
            return None;
        }

        let paddr = match unsafe { self.inner.alloc(len / MIN_PAGE_SIZE) } {
            Ok(addr) => addr,
            Err(err) => {
                trace!("failed to allocate from page allocator: {err:?}");
                return None;
            }
        };

        let paddr = PAddr::new(paddr);
        match page_type {
            PageType::Dirty => {}
            PageType::Zeroed => {
                let vaddr = arch::paddr2vaddr(paddr);
                let ptr = vaddr.as_usize() as *mut u8;
                unsafe {
                    core::ptr::write_bytes(ptr, 0, len);
                }
            }
        }

        Some(paddr)
    }

    /// Frees memory pages.
    ///
    /// # Safety
    ///
    /// The caller must provide a pair of `paddr` and `len` that are allocated
    /// from this allocator.
    pub unsafe fn free(&self, paddr: PAddr, len: usize) {
        if !is_aligned(len, MIN_PAGE_SIZE) {
            trace!("tried to free unaligned size: {len}");
            return;
        }

        let num_pages = len / MIN_PAGE_SIZE;
        if let Err(err) = unsafe { self.inner.free(paddr.as_usize(), num_pages) } {
            trace!("failed to free memory pages: {err:?}");
        }
    }
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

/// A simple bubble sort implementation.
///
/// This is used because `sort_unstable_by_key` is a big function in .text,
/// and free RAM regions are usually not many.
fn bubble_sort<T, F>(slice: &mut [T], mut f: F)
where
    F: FnMut(&T, &T) -> bool,
{
    let len = slice.len();
    for i in 0..len {
        for j in 0..len - i - 1 {
            if f(&slice[j], &slice[j + 1]) {
                slice.swap(j, j + 1);
            }
        }
    }
}

pub fn init(bootinfo: &mut BootInfo) {
    bubble_sort(bootinfo.reserved_regions.as_slice_mut(), |a, b| {
        a.start > b.start
    });

    // Visit the free RAM regions and add them to the page allocator.
    for FreeRam { addr, size } in &bootinfo.free_rams {
        let Some(end) = addr.as_usize().checked_add(*size).map(PAddr::new) else {
            trace!("free RAM region overflows: {addr} + {}", ByteSize(*size));
            continue;
        };

        // QEMU does not exclude module regions from the free RAM regions. Exclude
        // them manually so that the kernel won't try to allocate from them.
        visit_unused_regions(
            *addr,
            end,
            bootinfo.reserved_regions.as_slice(),
            |addr, end| {
                let size = end.as_usize() - addr.as_usize();
                trace!("RAM: {addr} - {end} ({})", ByteSize(size));
                PAGE_ALLOCATOR.add_region(addr, end);
            },
        );
    }
}
