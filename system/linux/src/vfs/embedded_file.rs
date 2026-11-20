use core::cmp::min;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFREG;
use crate::types::sys::stat::Stat;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;

/// A file that is embedded into the system's binary.
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

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(self.data.len())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        let ino = self.data.as_ptr().addr() as u64;
        Ok(Stat::new(ino, S_IFREG | 0o755, self.data.len()))
    }
}
