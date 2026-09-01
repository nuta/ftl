use alloc::sync::Arc;

use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;
use crate::wait_queue::WaitQueue;

mod console;
mod embedded_file;

pub use console::Console;
pub use embedded_file::EmbeddedFile;

pub trait FileLike: Send + Sync {
    fn bind(&self, addr: SockAddr) -> Result<(), Errno> {
        let _ = addr;
        Err(Errno::ENOTSUP)
    }

    fn listen(&self, backlog: c_int) -> Result<(), Errno> {
        let _ = backlog;
        Err(Errno::ENOTSUP)
    }

    fn accept(&self) -> Result<Arc<dyn FileLike>, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn close(&self) {}

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

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(0)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        None
    }
}
