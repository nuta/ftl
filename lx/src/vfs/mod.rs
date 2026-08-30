use core::any::Any;

use crate::types::errno::Errno;

mod console;
mod embedded_file;

pub use console::Console;
pub use embedded_file::EmbeddedFile;

pub trait FileLike: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        Err(Errno::ENOTSUP)
    }

    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        Err(Errno::ENOTSUP)
    }
}
