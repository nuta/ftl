use core::cmp::max;
use core::ops::Range;

use crate::address::VAddr;
use crate::arch::x64::vmspace::vaddr2paddr;

unsafe extern "C" {
    static __kernel_memory: u8;
    static __kernel_memory_end: u8;
}

pub(super) fn kernel_memory_range() -> Range<u64> {
    let kernel_memory_start = VAddr::new(&raw const __kernel_memory as usize);
    let kernel_memory_end = VAddr::new(&raw const __kernel_memory_end as usize);
    vaddr2paddr(kernel_memory_start).as_u64()..vaddr2paddr(kernel_memory_end).as_u64()
}

pub(super) fn reserved_regions_with_initfs(initfs_range: Range<u64>) -> [Range<u64>; 2] {
    let mut reserved_regions = [kernel_memory_range(), initfs_range];
    reserved_regions.sort_unstable_by_key(|r| r.start);
    reserved_regions
}

pub(super) fn exclude_reserved_regions(
    free_start: u64,
    free_end: u64,
    reserved_regions: &[Range<u64>],
    mut f: impl FnMut(u64, u64),
) {
    debug_assert!(reserved_regions.is_sorted_by_key(|r| r.start));

    if reserved_regions.is_empty() {
        f(free_start, free_end);
        return;
    }

    let mut current = free_start;
    for range in reserved_regions {
        if range.start >= free_end {
            // The reserved region is after the free region.
            break;
        }

        if range.start > current {
            // Found a gap.
            f(current, range.start);
        }

        current = max(current, range.end);
    }

    if current < free_end {
        // The remaining gap.
        f(current, free_end);
    }
}
