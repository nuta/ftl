use alloc::sync::Arc;
use core::slice;

use crate::types;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;
use crate::wait_queue::WaitQueue;

mod console;
mod embedded_file;
mod epoll;
mod eventfd;
mod pipe;
mod tty;

pub use console::Console;
pub use embedded_file::EmbeddedFile;
pub use epoll::Epoll;
pub use eventfd::EventFd;
pub use pipe::Pipe;

pub struct IoVec<'a> {
    slice: &'a [u8],
}
pub use tty::Tty;

pub struct IoVecSlice<'a> {
    iovecs: &'a [types::sys::uio::IoVec],
}

impl<'a> IoVecSlice<'a> {
    pub fn new(iov: *const types::sys::uio::IoVec, count: usize) -> Self {
        let iovecs = unsafe { slice::from_raw_parts(iov, count) };
        Self { iovecs }
    }

    pub fn total_len(&self) -> usize {
        let mut total = 0;
        for iovec in self.iovecs {
            total += iovec.iov_len;
        }
        total
    }

    pub fn buffers(&self) -> impl Iterator<Item = &[u8]> {
        self.iovecs.iter().map(|iovec| unsafe {
            slice::from_raw_parts(iovec.iov_base as *const u8, iovec.iov_len)
        })
    }

    pub fn buffers_mut(&mut self) -> impl Iterator<Item = &mut [u8]> {
        self.iovecs.iter().map(|iovec| unsafe {
            slice::from_raw_parts_mut(iovec.iov_base as *mut u8, iovec.iov_len)
        })
    }
}

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

    fn sendto(
        &self,
        buf: &[u8],
        dest: Option<SockAddr>,
        flags: c_int,
        nonblocking: bool,
    ) -> Result<usize, Errno> {
        let _ = (buf, dest, flags, nonblocking);
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

    fn writev(
        &self,
        iovecs: &IoVecSlice,
        offset: usize,
        nonblocking: bool,
    ) -> Result<usize, Errno> {
        let mut total = 0;
        for buf in iovecs.buffers() {
            match self.write(buf, offset + total, nonblocking) {
                Ok(n) if n < buf.len() => {
                    // Partial write.
                    total += n;
                    break;
                }
                Ok(n) => {
                    total += n;
                }
                Err(_) if total > 0 => {
                    // Write failed this time, but some data were successfully
                    // written.
                    break;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(total)
    }

    fn size(&self) -> Result<usize, Errno> {
        Err(Errno::ESPIPE)
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
