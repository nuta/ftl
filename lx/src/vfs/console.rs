use ftl::info;

use crate::types::errno::Errno;
use crate::vfs::FileLike;

pub struct Console {
    _private: (),
}

impl Console {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl FileLike for Console {
    fn read(&self, _buf: &mut [u8], _offset: usize, _nonblocking: bool) -> Result<usize, Errno> {
        todo!()
    }

    fn write(&self, buf: &[u8], _offset: usize, _nonblocking: bool) -> Result<usize, Errno> {
        if let Ok(s) = core::str::from_utf8(buf) {
            info!("[console] {}", s.trim_ascii_end());
        } else {
            info!("[console] invalid UTF-8");
        }

        Ok(buf.len())
    }
}
