use alloc::sync::Arc;
use core::ptr;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::stat::S_IFCHR;
use crate::types::sys::stat::Stat;
use crate::vfs::DirEntry;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::MemDir;
use crate::wait_queue::Sleep;

/// `/dev/null`.
pub struct Null {
    _private: (),
}

impl Null {
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl FileLike for Null {
    fn read(
        &self,
        _buf: &mut [u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        // Always EOF.
        Ok(0)
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        // Discard the data.
        Ok(buf.len())
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(0)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLIN | POLLOUT)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(Stat::new(
            ptr::from_ref(self).addr() as u64,
            S_IFCHR | 0o666,
            0,
        ))
    }
}

/// `/dev/zero`.
struct Zero;

impl FileLike for Zero {
    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        // Discard the data.
        Ok(buf.len())
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(0)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLIN | POLLOUT)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(Stat::new(
            ptr::from_ref(self).addr() as u64,
            S_IFCHR | 0o666,
            0,
        ))
    }
}

/// `/dev/urandom`.
struct URandom;

impl FileLike for URandom {
    fn read(
        &self,
        buf: &mut [u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        crate::random::read(buf)?;
        Ok(buf.len())
    }

    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        // Discard the data.
        Ok(buf.len())
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(0)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLIN | POLLOUT)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(Stat::new(
            ptr::from_ref(self).addr() as u64,
            S_IFCHR | 0o666,
            0,
        ))
    }
}

/// The device file system (`/dev`).
pub struct DevFs {
    _private: (),
}

impl DevFs {
    pub fn new() -> MemDir {
        MemDir::with_entries([
            DirEntry::new(b"null", INode::File(Arc::new(Null::new()))),
            DirEntry::new(b"zero", INode::File(Arc::new(Zero))),
            DirEntry::new(b"urandom", INode::File(Arc::new(URandom))),
        ])
    }
}
