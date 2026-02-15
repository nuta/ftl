use core::mem::size_of;
use core::ptr::read_volatile;
use core::ptr::write_volatile;
use core::sync::atomic::Ordering;
use core::sync::atomic::fence;

use ftl::driver::DmaBuf;
use ftl::log::trace;
use ftl::prelude::*;
use ftl_utils::alignment::align_up;

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FullError;

pub enum ChainEntry {
    Write { paddr: u64, len: u32 },
    Read { paddr: u64, len: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeadId(pub u16);

pub struct UsedChain {
    pub head: HeadId,
    pub total_len: u32,
}

pub struct PushToken {
    head_index: u16,
}

pub struct VirtQueue<C> {
    queue_size: u16,
    queue_index: u16,
    _dmabuf: DmaBuf,
    descs: *mut Desc,
    avail: *mut Avail,
    used: *mut Used,
    free_indicies: Vec<u16>,
    last_used_idx: u16,
    contexts: Vec<Option<C>>,
}

impl<C> VirtQueue<C> {
    pub fn new(queue_index: u16, queue_size: u16, dmabuf: DmaBuf) -> Self {
        let vaddr = dmabuf.vaddr();
        let descs = vaddr as *mut Desc;
        let avail_offset = size_of::<Desc>() * queue_size as usize;
        let avail = (vaddr + avail_offset) as *mut Avail;
        let used_offset = align_up(
            avail_offset + size_of::<u16>() * (2 + queue_size as usize),
            4096,
        );
        let used = (vaddr + used_offset) as *mut Used;
        let mut free_indicies = Vec::with_capacity(queue_size as usize);
        for index in 0..queue_size {
            free_indicies.push(index);
        }
        unsafe {
            (*avail).flags = 0;
            (*avail).idx = 0;
        }

        let mut contexts = Vec::with_capacity(queue_size as usize);
        for _ in 0..queue_size {
            contexts.push(None);
        }

        Self {
            queue_index,
            queue_size,
            _dmabuf: dmabuf,
            descs,
            avail,
            used,
            free_indicies,
            last_used_idx: 0,
            contexts,
        }
    }

    pub fn queue_size(&self) -> usize {
        self.queue_size as usize
    }

    pub fn queue_index(&self) -> u16 {
        self.queue_index
    }

    pub fn can_push(&self) -> bool {
        !self.free_indicies.is_empty()
    }

    pub fn can_pop(&self) -> bool {
        fence(Ordering::Acquire);

        let used_idx = unsafe { read_volatile(&(*self.used).idx) };
        self.last_used_idx != used_idx
    }

    pub fn reserve(&mut self) -> Option<PushToken> {
        let head_index = self.free_indicies.pop()?;
        Some(PushToken { head_index })
    }

    /// Push a descriptor chain to the available ring.
    pub fn push(&mut self, ticket: PushToken, chain: &[ChainEntry], ctx: C) {
        assert!(chain.len() > 0);

        // Add descriptors to the chain.
        let mut next_index = None;
        for (i, entry) in chain.iter().enumerate() {
            let desc_index = if let Some(index) = next_index {
                index
            } else {
                // The first descriptor in the chain.
                ticket.head_index
            };

            let (mut flags, paddr, len) = match entry {
                ChainEntry::Write { paddr, len } => (DESC_F_WRITE, *paddr, *len),
                ChainEntry::Read { paddr, len } => (0, *paddr, *len),
            };

            let next = if i < chain.len() - 1 {
                // More entries to come. Prepare the next index.
                flags |= DESC_F_NEXT;
                self.free_indicies.pop().unwrap()
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
                self.descs.offset(desc_index as isize).write(desc);
            }

            next_index = Some(next);
        }

        // Write the head index to the avail ring.
        let avail_index = unsafe { read_volatile(&(*self.avail).idx) };
        let ring_index = (avail_index % self.queue_size) as usize;
        unsafe {
            write_volatile(
                (*self.avail).ring.as_mut_ptr().add(ring_index),
                ticket.head_index,
            );
        }
        fence(Ordering::Release);
        unsafe {
            write_volatile(&mut (*self.avail).idx, avail_index.wrapping_add(1));
        }

        debug_assert!(self.contexts[ticket.head_index as usize].is_none());
        self.contexts[ticket.head_index as usize] = Some(ctx);
    }

    /// Pops a used descriptor chain (i.e. a complete request).
    ///
    /// Returns `(ctx, total_len)`, where `total_len` is the total length
    /// written by the device.
    pub fn pop(&mut self) -> Option<(C, usize)> {
        if !self.can_pop() {
            return None;
        }

        let index = (self.last_used_idx % self.queue_size) as usize;
        let elem = unsafe { read_volatile((*self.used).ring.as_ptr().add(index)) };
        self.last_used_idx = self.last_used_idx.wrapping_add(1);

        // Return all descriptors in the chain to the free pool.
        let mut index = elem.id as u16;
        let mut count = 0;
        loop {
            if count >= self.queue_size {
                // Too long chain. This should never happen, but it's not
                // critical enough to panic. Just log it.
                trace!("virtio: too long chain detected");
                break;
            }

            self.free_indicies.push(index);
            let desc = unsafe { read_volatile(self.descs.add(index as usize)) };
            count += 1;
            if desc.flags & DESC_F_NEXT == 0 {
                break;
            }
            index = desc.next;
        }

        let Some(ctx) = self.contexts[index as usize].take() else {
            warn!("context not found for index {index}");
            return None;
        };

        Some((ctx, elem.len as usize))
    }
}
