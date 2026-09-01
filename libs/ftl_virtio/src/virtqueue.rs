use core::mem::size_of;
use core::ptr::read_volatile;
use core::ptr::write_volatile;
use core::sync::atomic::Ordering;
use core::sync::atomic::fence;

use ftl_arrayvec::ArrayVec;
use ftl_driver::dma::DmaBuf;
use ftl_utils::alignment::align_up;

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;

pub(crate) const MAX_QUEUE_SIZE: usize = 256;

#[repr(C, packed)]
pub(crate) struct Desc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct UsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct Avail {
    flags: u16,
    idx: u16,
    ring: [u16; 0],
}

#[repr(C)]
struct Used {
    flags: u16,
    idx: u16,
    ring: [UsedElem; 0],
}

#[derive(Debug)]
pub enum ChainEntry {
    Write { paddr: u64, len: u32 },
    Read { paddr: u64, len: u32 },
}

#[derive(Debug)]
pub enum PushError {
    ZeroDescs,
    TooManyDescs,
}

#[derive(Debug)]
pub enum PopError {
    BadIndex,
    FreeListFull,
    ContextNotSet,
}

pub struct VirtQueue<C> {
    queue_size: u16,
    queue_index: u16,
    dmabuf: DmaBuf,
    avail_offset: usize,
    used_offset: usize,
    free_indices: ArrayVec<u16, MAX_QUEUE_SIZE>,
    last_used_idx: u16,
    contexts: [Option<C>; MAX_QUEUE_SIZE],
}

impl<C> VirtQueue<C> {
    pub(crate) fn new(queue_index: u16, queue_size: u16, mut dmabuf: DmaBuf) -> Self {
        let avail_offset = size_of::<Desc>() * queue_size as usize;
        let used_offset = align_up(
            avail_offset + size_of::<u16>() * (2 + queue_size as usize),
            4096,
        );

        let mut free_indices = ArrayVec::new();
        for index in 0..queue_size {
            free_indices.try_push(index).unwrap();
        }

        let avail =
            unsafe { &mut *(dmabuf.as_mut_slice().as_mut_ptr().add(avail_offset) as *mut Avail) };
        avail.flags = 0;
        avail.idx = 0;

        Self {
            queue_index,
            queue_size,
            dmabuf,
            avail_offset,
            used_offset,
            free_indices,
            last_used_idx: 0,
            contexts: [const { None }; MAX_QUEUE_SIZE],
        }
    }

    pub fn queue_size(&self) -> usize {
        self.queue_size as usize
    }

    pub fn queue_index(&self) -> u16 {
        self.queue_index
    }

    pub fn can_push(&self) -> bool {
        !self.free_indices.is_empty()
    }

    pub fn can_pop(&self) -> bool {
        let used = unsafe { self.dmabuf.as_slice().as_ptr().add(self.used_offset) as *mut Used };
        let used_idx = unsafe { read_volatile(&(*used).idx) };
        if self.last_used_idx == used_idx {
            return false;
        }

        fence(Ordering::Acquire);
        true
    }

    /// Push a descriptor chain to the available ring.
    pub fn push(&mut self, chain: &[ChainEntry], ctx: C) -> Result<(), (PushError, C)> {
        if chain.is_empty() {
            return Err((PushError::ZeroDescs, ctx));
        }

        if chain.len() > self.free_indices.len() {
            return Err((PushError::TooManyDescs, ctx));
        }

        // SAFETY: The length check above guarantees enough free descriptors.
        let head_index = self.free_indices.pop().unwrap();

        // Add descriptors to the chain.
        let mut next_index = None;
        for (i, entry) in chain.iter().enumerate() {
            let desc_index = if let Some(index) = next_index {
                index
            } else {
                // The first descriptor in the chain.
                head_index
            };

            let (mut flags, paddr, len) = match entry {
                ChainEntry::Write { paddr, len } => (DESC_F_WRITE, *paddr, *len),
                ChainEntry::Read { paddr, len } => (0, *paddr, *len),
            };

            let next = if i < chain.len() - 1 {
                // More entries to come. Prepare the next index.
                flags |= DESC_F_NEXT;
                self.free_indices.pop().unwrap()
            } else {
                // This is the last descriptor in the chain.
                0
            };

            let desc = Desc {
                addr: paddr,
                len,
                flags,
                next,
            };

            unsafe {
                let descs = self.dmabuf.as_mut_slice().as_mut_ptr().add(0) as *mut Desc;
                descs.offset(desc_index as isize).write(desc);
            }

            next_index = Some(next);
        }

        let avail = unsafe {
            self.dmabuf
                .as_mut_slice()
                .as_mut_ptr()
                .add(self.avail_offset) as *mut Avail
        };

        // Write the head index to the avail ring.
        let avail_index = unsafe { read_volatile(&(*avail).idx) };
        let ring_index = (avail_index % self.queue_size) as usize;
        unsafe {
            write_volatile((*avail).ring.as_mut_ptr().add(ring_index), head_index);
        }
        fence(Ordering::Release);
        unsafe {
            write_volatile(&mut (*avail).idx, avail_index.wrapping_add(1));
        }

        debug_assert!(self.contexts[head_index as usize].is_none());
        self.contexts[head_index as usize] = Some(ctx);
        Ok(())
    }

    /// Pops a used descriptor chain (i.e. a complete request).
    ///
    /// Returns `(ctx, total_len)`, where `total_len` is the total length
    /// written by the device.
    pub fn pop(&mut self) -> Result<Option<(C, usize)>, PopError> {
        if !self.can_pop() {
            return Ok(None);
        }

        let used = unsafe {
            self.dmabuf
                .as_mut_slice()
                .as_mut_ptr()
                .add(self.used_offset) as *mut Used
        };

        let index = (self.last_used_idx % self.queue_size) as usize;
        let elem = unsafe { read_volatile((*used).ring.as_ptr().add(index)) };
        self.last_used_idx = self.last_used_idx.wrapping_add(1);

        let head = elem.id as u16;
        let ctx = match self.contexts.get_mut(head as usize) {
            Some(ctx) => {
                match ctx.take() {
                    Some(ctx) => ctx,
                    // The context is not set for this index. Could be a bug
                    // in this driver, or in the device.
                    None => return Err(PopError::ContextNotSet),
                }
            }
            // The index is out of bounds. The device returned a bad index.
            None => return Err(PopError::BadIndex),
        };

        let descs = unsafe { self.dmabuf.as_mut_slice().as_mut_ptr().add(0) as *mut Desc };

        // Return all descriptors in the chain to the free pool.
        let mut index = head;
        let mut count = 0;
        loop {
            if count >= self.queue_size {
                // Too long chain. Should never happen.
                break;
            }

            if self.free_indices.try_push(index).is_err() {
                // Too many descriptors. This should not happen, but if it
                // does, ignore it.
                return Err(PopError::FreeListFull);
            }

            let desc = unsafe { read_volatile(descs.add(index as usize)) };
            count += 1;
            if desc.flags & DESC_F_NEXT == 0 {
                break;
            }
            index = desc.next;
        }

        Ok(Some((ctx, elem.len as usize)))
    }
}
