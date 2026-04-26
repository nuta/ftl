use alloc::vec::Vec;
use core::cmp::min;

pub struct RingBuffer {
    buf: Vec<u8>,
}

impl RingBuffer {
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn writeable_len(&self) -> usize {
        4096 // FIXME: capacity
    }

    pub fn write_bytes(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    pub fn read_bytes<F>(&mut self, max_len: usize,  f: F)
    where 
    F: FnOnce(&[u8]) -> usize,
    {
        let read_len = f(&self.buf[..min(max_len, self.buf.len())]);
        self.buf.drain(..read_len);
    }
}
