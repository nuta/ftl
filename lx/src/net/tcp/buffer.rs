use alloc::vec::Vec;
use core::cmp::min;

use ftl_types::error::ErrorCode;

pub(super) const TCP_BUFFER_CAPACITY: usize = 4096;

pub(super) struct TcpBuffer {
    bytes: Vec<u8>,
}

impl TcpBuffer {
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn readable_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn writable_len(&self) -> usize {
        TCP_BUFFER_CAPACITY.saturating_sub(self.bytes.len())
    }

    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let len = min(bytes.len(), self.writable_len());
        self.bytes.extend_from_slice(&bytes[..len]);
        len
    }

    pub fn receive<F>(&mut self, len: usize, receive: F) -> Result<bool, ErrorCode>
    where
        F: FnOnce(&mut [u8]) -> Result<(), ErrorCode>,
    {
        if len > self.writable_len() {
            return Ok(false);
        }
        let old_len = self.bytes.len();
        self.bytes
            .try_reserve(len)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        self.bytes.resize(old_len + len, 0);
        if let Err(error) = receive(&mut self.bytes[old_len..]) {
            self.bytes.truncate(old_len);
            return Err(error);
        }
        Ok(true)
    }

    pub fn read(&mut self, output: &mut [u8]) -> usize {
        let len = min(output.len(), self.readable_len());
        output[..len].copy_from_slice(&self.bytes[..len]);
        self.consume(len);
        len
    }

    pub fn peek_from(&self, offset: usize, max_len: usize) -> Option<&[u8]> {
        if max_len == 0 || offset >= self.readable_len() {
            return None;
        }

        let end = min(offset.saturating_add(max_len), self.readable_len());
        Some(&self.bytes[offset..end])
    }

    pub fn consume(&mut self, len: usize) {
        let len = min(len, self.readable_len());
        self.bytes.drain(..len);
    }
}
