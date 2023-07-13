use core::{
    hint::unreachable_unchecked,
    mem::{align_of, size_of, MaybeUninit},
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
use essentials::{
    alignment::{align_up, is_aligned},
    static_assert,
};

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
    /// Process.
    Process(SharedRefInner<Process>),
}

const fn object_size<T>() -> usize {
    align_up(size_of::<T>(), PAGE_SIZE)
}

const fn object_align<T>() -> usize {
    align_up(align_of::<T>(), PAGE_SIZE)
}

// TODO: Move this to a test.
static_assert!(object_size::<PageTable>() == PAGE_SIZE);
static_assert!(object_align::<PageTable>() == PAGE_SIZE);
static_assert!(object_size::<Process>() == PAGE_SIZE);
static_assert!(object_align::<Process>() == PAGE_SIZE);

#[derive(Debug)]
pub enum RetypeError {
    UnalignedAddress,
    InvalidLength,
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

    fn initialize<F, G, T>(
        &mut self,
        vaddr: VAddr,
        len: usize,
        object_ctor: F,
        frame_ctor: G,
    ) -> Result<&mut Frame, RetypeError>
    where
        F: FnOnce() -> T,
        G: FnOnce(NonNull<T>) -> Frame,
    {
        if !is_aligned(vaddr.as_usize(), object_align::<T>()) {
            return Err(RetypeError::UnalignedAddress);
        }

        if object_size::<T>() != len {
            return Err(RetypeError::InvalidLength);
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

        // Initialize the object and get the pointer to it.
        let mut object = unsafe {
            let mut uninit: &mut MaybeUninit<T> = vaddr.as_mut();
            uninit.write(object_ctor());

            // Now that the page table is initialized. It's safe to create a
            // pointer to it.
            NonNull::new_unchecked(uninit.as_ptr() as *mut T)
        };

        // Initialize the first frame.
        let first_frame = &mut frames[0];
        *first_frame = frame_ctor(object);
        Ok(first_frame)
    }

    pub fn initialize_page_table(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<PageTable>, RetypeError> {
        let first_frame = self.initialize(
            vaddr,
            len,
            || PageTable::new(),
            |object| {
                // Safety: We'll create a SharedRef for this below.
                Frame::PageTable(unsafe { SharedRefInner::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::PageTable(ref mut inner) => SharedRef::new(inner),
            // Safety: We just filled the first frame above.
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
        let first_frame = self.initialize(
            vaddr,
            len,
            || Process::new(pagetable),
            |object| {
                // Safety: We'll create a SharedRef for this below.
                Frame::Process(unsafe { SharedRefInner::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::Process(ref mut inner) => SharedRef::new(inner),
            // Safety: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }
}

static MEMORY_POOL: Option<GiantLock<MemoryPool>> = None;

pub fn memory_pool_mut(
    vaddr: VAddr,
    len: usize,
) -> Option<&'static GiantLock<MemoryPool>> {
    MEMORY_POOL.as_ref()
}

fn find_frame_by_vaddr(vaddr: VAddr) -> Option<&'static Frame> {
    todo!()
}

pub fn retype_frames_as_unused(vaddr: VAddr, num_pages: usize) {
    todo!();
}
