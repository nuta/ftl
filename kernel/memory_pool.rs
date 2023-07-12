use core::{
    hint::unreachable_unchecked,
    mem::{size_of, MaybeUninit},
    ops::Range,
    ptr::NonNull,
    slice,
};

use crate::{
    address::VAddr,
    arch::{PageTable, PAGE_SIZE},
    giant_lock::{GiantLock, GiantLockGuard},
    object::ObjectKind,
    process::Process,
    ref_count::{SharedRef, SharedRefInner, UniqueRef},
};
use essentials::alignment::{align_up, is_aligned};

/// A page frame.
enum Frame {
    /// A frame that is not being used.
    Unused,
    /// Not available. Used by kernel or reserved by hardware.
    Reserved,
    /// A frame that is a part of an object larger than a single frame.
    Continued {
        /// # of frames from the beginning of the first frame for this object.
        index: usize,
        /// Whether it's a last frame of the object.
        tail: bool,
    },
    /// Page table.
    PageTable(SharedRefInner<PageTable>),
}

#[derive(Debug)]
pub enum RetypeError {
    UnalignedAddress,
    OutOfRange,
    AlreadyInUse,
}

pub struct MemoryPool {
    base: VAddr,
    frames: &'static mut [Frame],
}

impl MemoryPool {
    pub fn new(vaddr: VAddr, len: usize) -> Option<MemoryPool> {
        debug_assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        debug_assert!(is_aligned(len, PAGE_SIZE));

        let num_frames = len / size_of::<Frame>();
        if num_frames * size_of::<Frame>() >= len {
            return None;
        }

        let frames = unsafe {
            slice::from_raw_parts_mut(vaddr.as_mut_ptr(), num_frames)
        };

        // FIXME: Optimize this initialization. We need something like memset.
        let num_control_frames =
            align_up(len * size_of::<Frame>(), PAGE_SIZE) / PAGE_SIZE;
        for frame in &mut frames[0..num_control_frames] {
            *frame = Frame::Reserved;
        }

        for frame in &mut frames[num_control_frames..] {
            *frame = Frame::Unused;
        }

        Some(MemoryPool {
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

    fn allocate<F, T>(
        &mut self,
        vaddr: VAddr,
        len: usize,
        ctor: F,
    ) -> Result<(&mut Frame, NonNull<T>), RetypeError>
    where
        F: FnOnce() -> T,
    {
        if !is_aligned(vaddr.as_usize(), PAGE_SIZE)
            || !is_aligned(len, PAGE_SIZE)
        {
            return Err(RetypeError::UnalignedAddress);
        }

        let range = self
            .frame_range(vaddr, len)
            .ok_or(RetypeError::OutOfRange)?;

        // Abort if any of the frames are already in use.
        let frames = &mut self.frames[range];
        if !frames.iter().all(|f| matches!(f, Frame::Unused)) {
            return Err(RetypeError::AlreadyInUse);
        }

        // Fill the frames after the first one.
        let num_frames = frames.len();
        for (index, frame) in frames[1..].iter_mut().enumerate() {
            *frame = Frame::Continued {
                index: index + 1,
                tail: index == num_frames - 1,
            };
        }

        // Initialize the page table and get the pointer to it.
        let mut inner = unsafe {
            let mut uninit: &mut MaybeUninit<T> = vaddr.as_mut();
            uninit.write(ctor());

            // Now that the page table is initialized. It's safe to create a
            // pointer to it.
            NonNull::new_unchecked(uninit.as_ptr() as *mut T)
        };

        Ok((&mut frames[0], inner))
    }

    pub fn initialize_page_table(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<PageTable>, RetypeError> {
        // FIXME: Check the `len` size.

        let (first_frame, inner) =
            self.allocate(vaddr, len, || PageTable::new())?;
        *first_frame = Frame::PageTable(SharedRefInner::new(inner));
        let sref = match first_frame {
            Frame::PageTable(ref mut inner) => SharedRef::new(inner),
            // Safety: We just filled the first frame with a PageTable.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }

    pub fn initialize_process(
        &mut self,
        vaddr: VAddr,
        len: usize,
        pagetable: UniqueRef<PageTable>,
    ) -> Result<SharedRef<Process>, RetypeError> {
        todo!()
    }
}

pub fn memory_pool(
    vaddr: VAddr,
    len: usize,
) -> Option<&'static GiantLock<MemoryPool>> {
    todo!()
}

fn find_frame_by_vaddr(vaddr: VAddr) -> Option<&'static Frame> {
    todo!()
}

pub fn retype_frames_as_unused(vaddr: VAddr, num_pages: usize) {
    todo!();
}
