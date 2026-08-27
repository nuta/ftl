use crate::types::errno::Errno;

mod console;
mod embedded_file;

pub use console::Console;
pub use embedded_file::EmbeddedFile;

pub trait FileLike: Send + Sync {
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
