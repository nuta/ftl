use alloc::sync::Arc;

use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;
use crate::wait_queue::WaitQueue;

mod console;
mod embedded_file;
mod epoll;
mod eventfd;

pub use console::Console;
pub use embedded_file::EmbeddedFile;
pub use epoll::Epoll;
pub use eventfd::EventFd;

pub trait FileLike: Send + Sync {
    fn bind(&self, addr: SockAddr) -> Result<(), Errno> {
        let _ = addr;
        Err(Errno::ENOTSUP)
    }

    fn listen(&self, backlog: c_int) -> Result<(), Errno> {
        let _ = backlog;
        Err(Errno::ENOTSUP)
    }

    fn accept(&self, nonblocking: bool) -> Result<Arc<dyn FileLike>, Errno> {
        let _ = nonblocking;
        Err(Errno::ENOTSUP)
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn recvfrom(
        &self,
        buf: &mut [u8],
        flags: c_int,
        nonblocking: bool,
    ) -> Result<(usize, SockAddr), Errno> {
        let _ = (buf, flags, nonblocking);
        Err(Errno::ENOTSOCK)
    }

    fn setsockopt(
        &self,
        level: c_int,
        optname: c_int,
        optval: *const u8,
        optlen: usize,
    ) -> Result<(), Errno> {
        let _ = (level, optname, optval, optlen);
        Err(Errno::ENOTSUP)
    }

    fn close(&self) {}

    fn read(&self, buf: &mut [u8], offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        let _ = nonblocking;
        Err(Errno::ENOTSUP)
    }

    fn write(&self, buf: &[u8], offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        let _ = nonblocking;
        Err(Errno::ENOTSUP)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(0)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        None
    }

    fn as_epoll(&self) -> Option<&Epoll> {
        None
    }
}
