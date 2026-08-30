use ftl::println;

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
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn read(&self, _buf: &mut [u8], _offset: usize) -> Result<usize, Errno> {
        todo!()
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, Errno> {
        if let Ok(s) = core::str::from_utf8(buf) {
            println!("[console] {}", s);
        } else {
            println!("[console] invalid UTF-8");
        }

        Ok(buf.len())
    }
}
