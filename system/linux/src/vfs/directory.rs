use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFDIR;
use crate::types::sys::stat::Stat;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::PathNode;
use crate::vfs::addr_as_ino;
use crate::wait_queue::Sleep;

/// An entry in a directory.
#[derive(Clone)]
pub struct DirEntry {
    pub name: Arc<[u8]>,
    pub inode: INode,
}

impl DirEntry {
    pub fn new(name: &[u8], inode: INode) -> Self {
        Self {
            name: name.into(),
            inode,
        }
    }
}

/// A `readdir` offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirOffset(usize);

impl DirOffset {
    pub const fn new(offset: usize) -> Self {
        Self(offset)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

pub trait Directory: Send + Sync {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno>;
    fn readdir(&self, offset: DirOffset) -> Result<Option<(DirEntry, DirOffset)>, Errno>;

    fn create(&self, name: &[u8]) -> Result<Arc<dyn FileLike>, Errno> {
        let _ = name;
        Err(Errno::EROFS)
    }

    fn mkdir(&self, name: &[u8]) -> Result<(), Errno> {
        let _ = name;
        Err(Errno::EROFS)
    }

    fn link(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        let _ = (name, inode);
        Err(Errno::EROFS)
    }

    fn unlink(&self, name: &[u8]) -> Result<(), Errno> {
        let _ = name;
        Err(Errno::EROFS)
    }

    fn rmdir(&self, name: &[u8]) -> Result<(), Errno> {
        let _ = name;
        Err(Errno::EROFS)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(Stat::new(addr_as_ino(self), S_IFDIR | 0o755, 0))
    }

    fn chmod(&self, mode: u32) -> Result<(), Errno> {
        let _ = mode;
        Err(Errno::EROFS)
    }

    fn fsync(&self) -> Result<(), Errno> {
        Err(Errno::EINVAL)
    }

    fn is_empty(&self) -> Result<bool, Errno> {
        let first_entry = self.readdir(DirOffset::new(0))?;
        Ok(first_entry.is_none())
    }
}

pub struct MemDir {
    entries: Vec<DirEntry>,
}

impl MemDir {
    pub fn with_entries(entries: impl Into<Vec<DirEntry>>) -> Self {
        Self {
            entries: entries.into(),
        }
    }
}

impl Directory for MemDir {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        for entry in self.entries.iter() {
            if *entry.name == *name {
                return Ok(entry.inode.clone());
            }
        }

        Err(Errno::ENOENT)
    }

    fn readdir(&self, offset: DirOffset) -> Result<Option<(DirEntry, DirOffset)>, Errno> {
        let Some(entry) = self.entries.get(offset.as_usize()) else {
            return Ok(None);
        };

        let next = DirOffset::new(offset.as_usize() + 1);
        Ok(Some((entry.clone(), next)))
    }
}

/// An opened directory.
pub struct OpenedDir {
    pnode: Arc<PathNode>,
    dir: Arc<dyn Directory>,
}

impl OpenedDir {
    pub fn new(pnode: Arc<PathNode>, dir: Arc<dyn Directory>) -> Self {
        Self { pnode, dir }
    }
}

impl FileLike for OpenedDir {
    fn read(
        &self,
        _buf: &mut [u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        Err(Errno::EISDIR)
    }

    fn readdir(&self, offset: DirOffset) -> Result<Option<(DirEntry, DirOffset)>, Errno> {
        self.dir.readdir(offset)
    }

    fn pnode(&self) -> Option<Arc<PathNode>> {
        Some(self.pnode.clone())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        self.pnode.inode().stat()
    }

    fn chmod(&self, mode: u32) -> Result<(), Errno> {
        self.dir.chmod(mode)
    }

    fn fsync(&self) -> Result<(), Errno> {
        self.dir.fsync()
    }
}
