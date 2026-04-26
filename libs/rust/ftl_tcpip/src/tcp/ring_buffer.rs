use alloc::vec::Vec;
use core::cmp::min;

pub struct RingBuffer {
    buf: Vec<u8>,
}

impl RingBuffer {
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn write_bytes(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    pub fn read_bytes(&mut self, buf: &mut [u8]) -> usize {
        let len = min(buf.len(), self.buf.len());
        buf[..len].copy_from_slice(&self.buf[..len]);
        self.buf.drain(..len);
        len
    }
}
