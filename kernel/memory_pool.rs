use core::{
    cell::OnceCell,
    hint::unreachable_unchecked,
    mem::{align_of, size_of, MaybeUninit},
    ops::Range,
    ptr::NonNull,
    slice,
};

use crate::{
    address::{PAddr, VAddr},
    arch::{
        Page4K, PageTable, PageTableL0, PageTableL1, PageTableL2, PAGE_SIZE,
    },
    backtrace,
    giant_lock::{GiantLock, GiantLockGuard},
    process::Process,
    ref_count::{SharedObject, SharedRef, UniqueRef},
};
use essentials::{
    alignment::{align_up, is_aligned},
    static_assert,
};

/// A page frame.
pub enum Frame {
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
    PageTable(SharedObject<PageTable>),
    PageTableL2(SharedObject<PageTableL2>),
    PageTableL1(SharedObject<PageTableL1>),
    PageTableL0(SharedObject<PageTableL0>),
    Page4K(SharedObject<Page4K>),
    /// Process.
    Process(SharedObject<Process>),
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

        let num_frames = len / PAGE_SIZE;
        if num_frames * size_of::<Frame>() >= len {
            return None;
        }

        let frames = unsafe {
            slice::from_raw_parts_mut(vaddr.as_mut_ptr(), num_frames)
        };

        // FIXME: Optimize this initialization. We need something like memset.
        let num_control_frames =
            align_up(num_frames * size_of::<Frame>(), PAGE_SIZE) / PAGE_SIZE;
        for frame in &mut frames[0..num_control_frames] {
            *frame = Frame::Reserved;
        }

        for frame in &mut frames[num_control_frames..] {
            *frame = Frame::Unused;
        }

        Some(MemoryPool {
            base: vaddr.offset(num_control_frames * PAGE_SIZE),
            frames,
        })
    }

    pub fn base(&self) -> VAddr {
        self.base
    }

    pub fn len(&self) -> usize {
        self.frames.len() * PAGE_SIZE
    }

    fn frame_index(&self, vaddr: VAddr) -> Option<usize> {
        if vaddr < self.base {
            return None;
        }

        Some((vaddr.as_usize() - self.base.as_usize()) / PAGE_SIZE)
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

    /// Frees an object.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///
    /// - `vaddr` points to a memory frame initialized by `initialize_*` methods.
    /// - `vaddr` is never freed more than once.
    pub unsafe fn free(&mut self, vaddr: VAddr) -> Result<(), RetypeError> {
        debug_assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));

        // Free the first frame.
        let index = self.frame_index(vaddr).ok_or(RetypeError::OutOfRange)?;

        // Free consecutive frames.
        self.frames[index] = Frame::Unused;
        if index + 1 < self.frames.len() {
            for frame in &mut self.frames[(index + 1)..] {
                match frame {
                    Frame::Continued { tail, .. } => {
                        let tail = *tail;
                        *frame = Frame::Unused;
                        if tail {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }

        Ok(())
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
                // SAFETY: We'll create a SharedRef for this below.
                Frame::PageTable(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::PageTable(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }

    pub fn initialize_page_table_l2(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<PageTableL2>, RetypeError> {
        let first_frame = self.initialize(
            vaddr,
            len,
            || PageTableL2::new(),
            |object| {
                // SAFETY: We'll create a SharedRef for this below.
                Frame::PageTableL2(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::PageTableL2(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }

    pub fn initialize_page_table_l1(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<PageTableL1>, RetypeError> {
        let first_frame = self.initialize(
            vaddr,
            len,
            || PageTableL1::new(),
            |object| {
                // SAFETY: We'll create a SharedRef for this below.
                Frame::PageTableL1(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::PageTableL1(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }

    pub fn initialize_page_table_l0(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<PageTableL0>, RetypeError> {
        let first_frame = self.initialize(
            vaddr,
            len,
            || PageTableL0::new(),
            |object| {
                // SAFETY: We'll create a SharedRef for this below.
                Frame::PageTableL0(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::PageTableL0(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }

    pub fn initialize_page4k(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<SharedRef<Page4K>, RetypeError> {
        let first_frame = self.initialize(
            vaddr,
            len,
            || Page4K::zeroed(),
            |object| {
                // SAFETY: We'll create a SharedRef for this below.
                Frame::Page4K(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::Page4K(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
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
                // SAFETY: We'll create a SharedRef for this below.
                Frame::Process(unsafe { SharedObject::new(object) })
            },
        )?;

        let sref = match first_frame {
            Frame::Process(object) => SharedRef::new(object),
            // SAFETY: We just filled the first frame above.
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(sref)
    }
}

static MEMORY_POOL: GiantLock<OnceCell<MemoryPool>> =
    GiantLock::new(OnceCell::new());

pub fn memory_pool_mut(
    vaddr: VAddr,
) -> Option<GiantLockGuard<'static, MemoryPool>> {
    crate::backtrace::backtrace();

    let pool_lock = MEMORY_POOL.borrow_mut();
    Some(GiantLockGuard::map(pool_lock, |pool| {
        match pool.get_mut() {
            Some(pool) => pool,
            None => {
                panic!("global memory pool is not initialized");
            }
        }
    }))
}

pub fn paddr2frame(paddr: PAddr) -> Option<GiantLockGuard<'static, Frame>> {
    let vaddr = paddr.vaddr()?;
    let guard = memory_pool_mut(vaddr)?;
    let index = guard.frame_index(vaddr)?;
    Some(GiantLockGuard::map(guard, |pool| &mut pool.frames[index]))
}

pub fn init(vaddr: VAddr, len: usize) {
    // TODO: Strictly speaking all vaddr + len pages are accessible by the kernel.
    //       Perhaps we should use UAddr?
    let pool = MemoryPool::new(vaddr, len)
        .expect("failed to initialize global memory pool");
    if let Err(_) = MEMORY_POOL.borrow_mut().set(pool) {
        panic!("global memory pool is already initialized");
    }
}
