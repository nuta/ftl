#![no_std]

extern crate alloc;

use core::{mem::size_of, slice};

use ::zerocopy::{AsBytes, FromBytes};
use alloc::boxed::Box;

pub mod zerocopy;

struct PacketHeader {
    head: usize,
    tail: usize,
    capacity: usize,
    #[allow(dead_code)]
    padding: u16,
}

pub struct PacketBuf {
    buf: *mut u8,
    buf_layout: core::alloc::Layout,
    // TODO: remove unnecessary fields
    header: &'static mut PacketHeader,
    data: &'static mut [u8],
}

impl PacketBuf {
    pub const HEADROOM: usize = 32;

    pub fn new(max_header_size: usize, max_payload_size: usize) -> PacketBuf {
        let capacity = max_header_size + max_payload_size;
        let buf_layout = core::alloc::Layout::from_size_align(
            size_of::<PacketHeader>() + capacity,
            core::mem::align_of::<PacketHeader>(),
        )
        .unwrap(); // TODO: no unwrap
        let buf = unsafe { alloc::alloc::alloc(buf_layout) };
        let header = unsafe { &mut *(buf as *mut PacketHeader) };

        header.head = max_header_size;
        header.tail = max_header_size;
        header.capacity = capacity;
        header.padding = 0;

        let data = {
            let header_ptr = header as *mut PacketHeader as *mut u8;

            // SAFETY: `offset` and `len` are checked to be within bounds.
            unsafe {
                slice::from_raw_parts_mut(header_ptr.add(size_of::<PacketHeader>()), capacity)
            }
        };

        PacketBuf {
            buf,
            buf_layout,
            header,
            data,
        }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[self.header.head..self.header.tail]
    }

    pub fn reserve_front(&mut self, len: usize) -> Option<&mut [u8]> {
        self.header.head = self.header.head.checked_sub(len)?;
        Some(&mut self.data[self.header.head..(self.header.head + len)])
    }

    pub fn reserve_back(&mut self, len: usize) -> Option<&mut [u8]> {
        let new_tail = self.header.tail + len;
        if new_tail > self.header.capacity {
            return None;
        }

        self.header.tail = new_tail;
        Some(&mut self.data[(self.header.tail - len)..self.header.tail])
    }

    pub fn prepend<T: FromBytes + AsBytes>(&mut self) -> Option<::zerocopy::Ref<&mut [u8], T>> {
        let buf = self.reserve_front(size_of::<T>())?;
        ::zerocopy::Ref::new(buf)
    }

    pub fn append<T: FromBytes + AsBytes>(&mut self) -> Option<::zerocopy::Ref<&mut [u8], T>> {
        let buf = self.reserve_back(size_of::<T>())?;
        ::zerocopy::Ref::new(buf)
    }

    pub fn truncate_front(&mut self, len: usize) -> Option<&mut [u8]> {
        let new_head = self.header.head + len;
        if new_head > self.header.tail {
            return None;
        }

        self.header.head = new_head;
        Some(&mut self.data[(self.header.head - len)..self.header.head])
    }

    pub fn pop_front<T: FromBytes + AsBytes>(&mut self) -> Option<::zerocopy::Ref<&[u8], T>> {
        let buf = self.truncate_front(size_of::<T>())?;
        ::zerocopy::Ref::new(buf)
    }
}

impl Drop for PacketBuf {
    fn drop(&mut self) {
        unsafe { alloc::alloc::dealloc(self.buf, self.buf_layout) }
    }
}
