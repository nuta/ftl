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
        // FIXME: Ensure the writeable len <= u16::MAX to use it as window size.
        4096 // FIXME: capacity
    }

    pub fn readable_len(&self) -> usize {
        self.buf.len()
    }

    pub fn write_bytes(&mut self, buf: &[u8]) -> usize {
        self.buf.extend_from_slice(buf);
        buf.len()
    }

    pub fn write_bytes_with<F>(&mut self, f: F) -> usize
    where
        F: FnOnce(&mut [u8]) -> usize,
    {
        let mut tmp = [0; 4096];
        let len = f(&mut tmp);
        self.write_bytes(&tmp[..len]);
        len
    }

    pub fn read_bytes_with<F>(&mut self, max_len: usize, f: F)
    where
        F: FnOnce(Option<&[u8]>) -> usize,
    {
        let buf = self.peek_bytes(max_len);
        let read_len = f(buf);
        self.consume_bytes(read_len);
    }

    pub fn peek_bytes(&mut self, max_len: usize) -> Option<&[u8]> {
        if self.readable_len() == 0 {
            return None;
        }

        Some(&self.buf[..min(max_len, self.readable_len())])
    }

    pub fn consume_bytes(&mut self, len: usize) {
        self.buf.drain(..min(len, self.readable_len()));
    }
}
