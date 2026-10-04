use alloc::sync::Arc;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLOUT;
use crate::vfs::Directory;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::wait_queue::Sleep;

/// `/dev/null`.
struct Null;

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

    fn size(&self) -> Result<usize, Errno> {
        Ok(0)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLIN | POLLOUT)
    }
}

/// The device file system (`/dev`).
pub struct DevFs {
    null: INode,
}

impl DevFs {
    pub fn new() -> Self {
        Self {
            null: INode::File(Arc::new(Null)),
        }
    }
}

impl Directory for DevFs {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        match name {
            b"null" => Ok(self.null.clone()),
            _ => Err(Errno::ENOENT),
        }
    }
}
