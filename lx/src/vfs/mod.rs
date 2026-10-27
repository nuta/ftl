use alloc::sync::Arc;
use core::cmp::min;
use core::slice;

use crate::types;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

mod console;
mod devfs;
mod directory;
mod embedded_file;
mod epoll;
mod eventfd;
mod inode;
mod lxfs;
mod path;
mod pipe;
mod procfs;
mod root_dir;
mod tty;

pub use console::Console;
pub use devfs::DevFs;
pub use devfs::Null;
pub use directory::DirEntry;
pub use directory::Directory;
pub use directory::OpenedDir;
pub use directory::StaticDir;
pub use embedded_file::EmbeddedFile;
pub use epoll::Epoll;
pub use eventfd::EventFd;
pub use inode::INode;
pub use lxfs::LxFs;
pub use path::PathNode;
pub use pipe::Pipe;
pub use procfs::ProcFs;
pub use root_dir::RootDir;
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
        self.iovecs
            .iter()
            .filter(|iovec| {
                // Buffer pointer may be null if iov_len is 0, but it is invalid
                // for Rust slices. Skip such iovecs.
                iovec.iov_len != 0
            })
            .map(|iovec| unsafe {
                slice::from_raw_parts(iovec.iov_base as *const u8, iovec.iov_len)
            })
    }

    pub fn copy_to(&self, mut dst: &mut [u8]) -> usize {
        let mut total_len = 0;
        for src in self.buffers() {
            if dst.is_empty() {
                break;
            }

            let (chunk, rest) = dst.split_at_mut(min(src.len(), dst.len()));
            chunk.copy_from_slice(&src[..chunk.len()]);
            total_len += chunk.len();
            dst = rest;
        }

        total_len
    }
}

pub trait FileLike: Send + Sync {
    /// Returns `Some(file)` for files that have per-open-file states.
    fn open(&self) -> Result<Option<Arc<dyn FileLike>>, Errno> {
        Ok(None)
    }

    fn bind(&self, addr: SockAddr) -> Result<(), Errno> {
        let _ = addr;
        Err(Errno::ENOTSUP)
    }

    fn listen(&self, backlog: c_int) -> Result<(), Errno> {
        let _ = backlog;
        Err(Errno::ENOTSUP)
    }

    fn accept(&self, nonblocking: bool, sleep: Sleep<'_>) -> Result<Arc<dyn FileLike>, Errno> {
        let _ = nonblocking;
        let _ = sleep;
        Err(Errno::ENOTSUP)
    }

    fn connect(&self, addr: SockAddr, nonblocking: bool, sleep: Sleep<'_>) -> Result<(), Errno> {
        let _ = (addr, nonblocking, sleep);
        Err(Errno::ENOTSOCK)
    }

    fn peer_addr(&self) -> Result<SockAddr, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn recvfrom(
        &self,
        buf: &mut [u8],
        flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        let _ = (buf, flags, nonblocking, sleep);
        Err(Errno::ENOTSOCK)
    }

    fn sendto(
        &self,
        buf: &[u8],
        dest: Option<SockAddr>,
        flags: c_int,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let _ = (buf, dest, flags, nonblocking, sleep);
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

    fn shutdown(&self, _how: c_int) -> Result<(), Errno> {
        Err(Errno::ENOTSOCK)
    }

    fn read(
        &self,
        buf: &mut [u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        let _ = nonblocking;
        let _ = sleep;
        Err(Errno::ENOTSUP)
    }

    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let _ = buf;
        let _ = offset;
        let _ = nonblocking;
        let _ = sleep;
        Err(Errno::ENOTSUP)
    }

    fn writev(
        &self,
        iovecs: &IoVecSlice,
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let mut total = 0;
        for buf in iovecs.buffers() {
            match self.write(buf, offset + total, nonblocking, sleep) {
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

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        let _ = index;
        Err(Errno::ENOTDIR)
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
