use core::{
    mem::{size_of, MaybeUninit},
    ops::Range,
    ptr::NonNull,
    slice,
};

use crate::{
    address::VAddr,
    arch::PAGE_SIZE,
    object::ObjectKind,
    ref_count::{SharedRef, SharedRefHeader},
};
use essentials::alignment::{align_up, is_aligned};

struct Frame {
    kind: ObjectKind,
    /// a memory space to construct `SharedRef<T>`.
    shared_ref: MaybeUninit<SharedRefHeader>,
}

impl Frame {
    const fn new(kind: ObjectKind) -> Frame {
        // Frame { kind, ref_count: 0 }
        todo!()
    }
}

enum RetypeError {
    UnalignedAddress,
    OutOfRange,
    AlreadyInUse,
}

struct MemoryPool {
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
            *frame = Frame::new(ObjectKind::Reserved);
        }

        for frame in &mut frames[num_control_frames..] {
            *frame = Frame::new(ObjectKind::Unused);
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

    pub fn allocate(
        &mut self,
        vaddr: VAddr,
        len: usize,
    ) -> Result<(), RetypeError> {
        if !is_aligned(vaddr.as_usize(), PAGE_SIZE)
            || !is_aligned(len, PAGE_SIZE)
        {
            return Err(RetypeError::UnalignedAddress);
        }

        let range = self
            .frame_range(vaddr, len)
            .ok_or(RetypeError::OutOfRange)?;

        let frames = &mut self.frames[range];
        if !frames.iter().all(|f| f.kind == ObjectKind::Unused) {
            return Err(RetypeError::AlreadyInUse);
        }

        // for frame in frames {
        //     debug_assert_eq!(frame.ref_count, 0);
        //     // frame.kind = kind;
        //     frame.ref_count = 1;
        // }

        // Ok()
        todo!()
    }
}

fn find_frame_by_vaddr(vaddr: VAddr) -> Option<&'static Frame> {
    todo!()
}

pub fn retype_frames_as_unused(vaddr: VAddr, num_pages: usize) {
    todo!();
}
