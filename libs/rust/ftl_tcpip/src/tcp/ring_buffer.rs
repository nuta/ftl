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

    pub fn readable_len(&self) -> usize {
        self.buf.len()
    }

    pub fn write_bytes(&mut self, buf: &[u8]) -> usize {
        self.buf.extend_from_slice(buf);
        buf.len()
    }

    pub fn peek_bytes(&mut self, max_len: usize) -> Option<&[u8]> {
        Some(&self.buf[..min(max_len, self.readable_len())])
    }

    pub fn consume_bytes(&mut self, len: usize) {
        self.buf.drain(..min(len, self.readable_len()));
    }
}
