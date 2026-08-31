use alloc::vec::Vec;
use core::cmp::min;

use ftl_types::error::ErrorCode;

pub(super) const TCP_BUFFER_SIZE: usize = 4096;

pub struct TcpBuffer {
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
        TCP_BUFFER_SIZE.saturating_sub(self.bytes.len())
    }

    /// Appends `bytes` to the buffer.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let len = min(bytes.len(), self.writable_len());
        self.bytes.extend_from_slice(&bytes[..len]);
        len
    }

    /// Appends up to `len` bytes to the buffer, using accessing its
    /// `&mut [u8]` slice directly.
    pub fn write_with<F>(&mut self, len: usize, writer: F) -> Result<usize, ErrorCode>
    where
        F: FnOnce(&mut [u8]) -> usize,
    {
        let copyable_len = min(len, self.writable_len());
        if copyable_len == 0 {
            return Ok(0);
        }

        // Try to reserve space for the new data.
        let old_len = self.bytes.len();
        self.bytes
            .try_reserve(copyable_len)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        self.bytes.resize(old_len + copyable_len, 0);

        // Let callback write the new data.
        let written_len = min(writer(&mut self.bytes[old_len..]), copyable_len);
        self.bytes.truncate(old_len + written_len);
        Ok(written_len)
    }

    /// Reads up to `len` bytes, and removes them from the buffer.
    pub fn read(&mut self, output: &mut [u8]) -> usize {
        let len = min(output.len(), self.readable_len());
        output[..len].copy_from_slice(&self.bytes[..len]);
        self.consume(len);
        len
    }

    /// Peeks up to `max_len` bytes from the buffer, starting at `offset`.
    pub fn peek(&self, offset: usize, max_len: usize) -> Option<&[u8]> {
        if max_len == 0 || offset >= self.readable_len() {
            return None;
        }

        let end = min(offset.saturating_add(max_len), self.readable_len());
        Some(&self.bytes[offset..end])
    }

    /// Discards first `len` bytes from the buffer.
    pub fn consume(&mut self, len: usize) {
        let len = min(len, self.readable_len());
        self.bytes.drain(..len);
    }
}
