use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    num::NonZeroUsize,
    ops::Range,
    ptr::{addr_of, NonNull},
    slice,
};

use crate::{address::VAddr, arch::PAGE_SIZE, giant_lock::GiantLock};

use bump_allocator::BumpAllocator;
use linked_list_allocator::Heap;
use utils::alignment::{align_up, is_aligned};

/// The size of the malloc heap in bytes. To be used in `core::alloc` objects.
const MALLOC_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
enum FrameKind {
    Unused,
    Reserved,
    Process,
    Thread,
    Channel,
    PageTableL0,
    PageTableL1,
    DataPage,
}

struct FrameControlBlock {
    kind: FrameKind,
    ref_count: usize,
}

impl FrameControlBlock {
    const fn new(kind: FrameKind) -> FrameControlBlock {
        FrameControlBlock { kind, ref_count: 0 }
    }
}

enum RetypeError {
    UnalignedAddress,
    OutOfRange,
    AlreadyUsed,
}

struct FrameZoneManager {
    base: VAddr,
    frames: &'static mut [FrameControlBlock],
}

impl FrameZoneManager {
    pub fn new(vaddr: VAddr, len: usize) -> Option<FrameZoneManager> {
        debug_assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        debug_assert!(is_aligned(len, PAGE_SIZE));

        let num_frames = len / size_of::<FrameControlBlock>();
        if num_frames * size_of::<FrameControlBlock>() >= len {
            return None;
        }

        let mut frames = unsafe {
            slice::from_raw_parts_mut(vaddr.as_mut_ptr(), num_frames)
        };

        // FIXME: Optimize this initialization. We need something like memset.
        let num_control_frames =
            align_up(len * size_of::<FrameControlBlock>(), PAGE_SIZE)
                / PAGE_SIZE;
        for frame in &mut frames[0..num_control_frames] {
            *frame = FrameControlBlock::new(FrameKind::Reserved);
        }

        for frame in &mut frames[num_control_frames..] {
            *frame = FrameControlBlock::new(FrameKind::Unused);
        }

        Some(FrameZoneManager {
            base: vaddr,
            frames,
        })
    }

    fn frame_range(&self, vaddr: VAddr, len: usize) -> Option<Range<usize>> {
        debug_assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        debug_assert!(is_aligned(len, PAGE_SIZE));

        if vaddr < self.base {
            return None;
        }

        let offset = vaddr.as_usize() - self.base.as_usize();
        let start = offset / PAGE_SIZE;
        let end = (offset + len) / PAGE_SIZE;
        if end > self.frames.len() {
            None
        } else {
            Some(start..end)
        }
    }

    pub fn retype(
        &mut self,
        vaddr: VAddr,
        len: usize,
        kind: FrameKind,
    ) -> Result<(), RetypeError> {
        if is_aligned(vaddr.as_usize(), PAGE_SIZE) || is_aligned(len, PAGE_SIZE)
        {
            return Err(RetypeError::UnalignedAddress);
        }

        let range = self
            .frame_range(vaddr, len)
            .ok_or(RetypeError::OutOfRange)?;

        let frames = &mut self.frames[range];
        if frames.iter().any(|f| f.kind != FrameKind::Unused) {
            return Err(RetypeError::AlreadyUsed);
        }

        for frame in frames {
            // TODO: constructor
            debug_assert_eq!(frame.ref_count, 0);
            frame.kind = kind;
            frame.ref_count = 1;
        }

        Ok(())
    }
}

static PAGE_ALLOCATOR: GiantLock<BumpAllocator> =
    GiantLock::new(BumpAllocator::new());

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator =
    HeapAllocator(GiantLock::new(Heap::empty()));

#[track_caller]
pub fn allocate_pages(size: usize) -> Option<NonZeroUsize> {
    PAGE_ALLOCATOR.borrow_mut().allocate(size, PAGE_SIZE)
}

pub fn allocate_all_pages() -> Option<(NonZeroUsize, usize)> {
    PAGE_ALLOCATOR.borrow_mut().allocate_all(PAGE_SIZE)
}

pub fn init() {
    extern "C" {
        static __boot_heap: u8;
        static __boot_heap_end: u8;
    }

    unsafe {
        let heap_start = addr_of!(__boot_heap) as usize;
        let heap_end = addr_of!(__boot_heap_end) as usize;

        // Initialize the boot-time page allocator.
        PAGE_ALLOCATOR
            .borrow_mut()
            .add_region(heap_start, heap_end - heap_start);

        // Initialize the malloc heap.
        let malloc_start = PAGE_ALLOCATOR
            .borrow_mut()
            .allocate(MALLOC_SIZE, size_of::<usize>())
            .map(|addr| addr.get() as *mut u8)
            .expect("failed to allocate malloc heap");
        HEAP_ALLOCATOR
            .0
            .borrow_mut()
            .init(malloc_start, MALLOC_SIZE);
    }
}

struct HeapAllocator(GiantLock<Heap>);

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .borrow_mut()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .borrow_mut()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}
