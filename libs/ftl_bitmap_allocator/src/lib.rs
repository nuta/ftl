#![no_std]

use core::ops::Range;

use ftl_arrayvec::ArrayVec;
use ftl_utils::alignment::is_aligned;
use ftl_utils::spinlock::SpinLock;

const NUM_REGIONS_MAX: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum AddRegionError {
    PointerNull,
    OutOfBounds,
    NotAligned,
    TooManyRegions,
    TooShort,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FreeError {
    NotFound,
    NotAligned,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AllocError {
    NoMemory,
}

struct Region {
    bitmap: *mut u8,
    paddr_start: usize,
    paddr_end: usize,
    num_pages: usize,
}

unsafe impl Send for Region {}

impl Region {
    fn find_free_pages(&self, n: usize) -> Option<Range<usize>> {
        let mut free_pages = 0;
        for i in 0..self.num_pages {
            let byte = unsafe { *self.bitmap.add(i / 8) };
            if byte & (1 << (i % 8)) != 0 {
                free_pages = 0;
                continue;
            }

            free_pages += 1;
            if free_pages == n {
                let start = i + 1 - n;
                return Some(start..i + 1);
            }
        }

        None
    }

    pub fn allocate(&mut self, page_size: usize, n: usize) -> Result<usize, AllocError> {
        if n == 0 || n > self.num_pages {
            return Err(AllocError::NoMemory);
        }

        let Some(range) = self.find_free_pages(n) else {
            return Err(AllocError::NoMemory);
        };

        // Mark the pages as allocated.
        for j in range.start..range.end {
            unsafe {
                *self.bitmap.add(j / 8) |= 1 << (j % 8);
            }
        }

        return Ok(self.paddr_start + range.start * page_size);
    }

    fn contains_paddr(&self, paddr: usize) -> bool {
        self.paddr_start <= paddr && paddr < self.paddr_end
    }

    pub fn free(&mut self, page_size: usize, paddr: usize, n: usize) -> Result<(), FreeError> {
        if !self.contains_paddr(paddr) {
            return Err(FreeError::NotFound);
        }

        // Mark the pages as free.
        let i = (paddr - self.paddr_start) / page_size;
        if n == 0 || n > self.num_pages - i {
            return Err(FreeError::NotFound);
        }
        for j in i..i + n {
            unsafe {
                *self.bitmap.add(j / 8) &= !(1 << (j % 8));
            }
        }

        Ok(())
    }
}

pub struct BitmapAllocator {
    regions: SpinLock<ArrayVec<Region, NUM_REGIONS_MAX>>,
    page_size: usize,
}

impl BitmapAllocator {
    pub const fn new(page_size: usize) -> Self {
        assert!(page_size.is_power_of_two());

        Self {
            regions: SpinLock::new(ArrayVec::new()),
            page_size,
        }
    }

    pub unsafe fn add_chunk(
        &self,
        ptr: *mut u8,
        paddr: usize,
        len: usize,
    ) -> Result<(), AddRegionError> {
        if ptr.is_null() {
            return Err(AddRegionError::PointerNull);
        }

        let Some(paddr_end) = paddr.checked_add(len) else {
            return Err(AddRegionError::OutOfBounds);
        };

        if !is_aligned(paddr, self.page_size) {
            return Err(AddRegionError::NotAligned);
        }

        if !is_aligned(len, self.page_size) {
            return Err(AddRegionError::NotAligned);
        }

        let total_pages = len / self.page_size;
        let bitmap_len = total_pages.div_ceil(8);
        let bitmap_pages = bitmap_len.div_ceil(self.page_size);
        if total_pages <= bitmap_pages {
            // The region is too short to reserve space for the bitmap.
            return Err(AddRegionError::TooShort);
        }

        let region = Region {
            bitmap: ptr,
            paddr_start: paddr + bitmap_pages * self.page_size,
            paddr_end,
            num_pages: total_pages - bitmap_pages,
        };

        let mut regions = self.regions.lock();
        if regions.try_push(region).is_err() {
            return Err(AddRegionError::TooManyRegions);
        }

        unsafe {
            ptr.write_bytes(0, bitmap_len);
        }

        Ok(())
    }

    pub unsafe fn alloc(&self, n: usize) -> Result<usize, AllocError> {
        let mut regions = self.regions.lock();
        for region in regions.iter_mut() {
            if let Ok(paddr) = region.allocate(self.page_size, n) {
                return Ok(paddr);
            }
        }

        Err(AllocError::NoMemory)
    }

    pub unsafe fn free(&self, paddr: usize, num_pages: usize) -> Result<(), FreeError> {
        if !is_aligned(paddr, self.page_size) {
            return Err(FreeError::NotAligned);
        }

        let mut regions = self.regions.lock();
        for region in regions.iter_mut() {
            match region.free(self.page_size, paddr, num_pages) {
                Ok(()) => return Ok(()),
                Err(FreeError::NotFound) => continue,
                Err(err) => return Err(err),
            }
        }

        Err(FreeError::NotFound)
    }
}
