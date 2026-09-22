use core::cmp::min;

use crate::types::errno::Errno;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;

/// A file that is embedded into LX's binary.
pub struct EmbeddedFile {
    data: &'static [u8],
}

impl EmbeddedFile {
    pub const fn new(data: &'static [u8]) -> Self {
        Self { data }
    }
}

impl FileLike for EmbeddedFile {
    fn read(
        &self,
        buf: &mut [u8],
        offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let len = self.data.len();
        if offset >= len {
            return Ok(0);
        }

        let copy_len = min(buf.len(), len - offset);
        buf[..copy_len].copy_from_slice(&self.data[offset..offset + copy_len]);
        Ok(copy_len)
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(self.data.len())
    }
}
