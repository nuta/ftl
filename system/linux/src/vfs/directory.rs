use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::types::errno::Errno;
use crate::types::sys::stat::Stat;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::PathNode;
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

pub trait Directory: Send + Sync {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno>;
    fn readdir(&self, offset: usize) -> Result<Option<(DirEntry, usize)>, Errno>;

    fn create(&self, name: &[u8]) -> Result<Arc<dyn FileLike>, Errno> {
        let _ = name;
        Err(Errno::EROFS)
    }

    fn mkdir(&self, name: &[u8]) -> Result<(), Errno> {
        let _ = name;
        Err(Errno::EROFS)
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

    fn readdir(&self, offset: usize) -> Result<Option<(DirEntry, usize)>, Errno> {
        let Some(entry) = self.entries.get(offset) else {
            return Ok(None);
        };

        Ok(Some((entry.clone(), offset + 1)))
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

    fn readdir(&self, offset: usize) -> Result<Option<(DirEntry, usize)>, Errno> {
        self.dir.readdir(offset)
    }

    fn pnode(&self) -> Option<Arc<PathNode>> {
        Some(self.pnode.clone())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        self.pnode.inode().stat()
    }
}
