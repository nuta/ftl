use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::types::errno::Errno;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::PathNode;
use crate::wait_queue::Sleep;

/// An entry in a directory.
#[derive(Clone)]
pub struct DirEntry {
    pub name: &'static [u8],
    pub inode: INode,
}

impl DirEntry {
    pub fn new(name: &'static [u8], inode: INode) -> Self {
        Self { name, inode }
    }
}

pub trait Directory: Send + Sync {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno>;
    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno>;
}

pub struct MemDir {
    entries: Vec<DirEntry>,
}

impl MemDir {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_entries(entries: impl Into<Vec<DirEntry>>) -> Self {
        Self {
            entries: entries.into(),
        }
    }

    pub fn add(&mut self, entry: DirEntry) {
        self.entries.push(entry);
    }
}

impl Directory for MemDir {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        for entry in self.entries.iter() {
            if entry.name == name {
                return Ok(entry.inode.clone());
            }
        }

        Err(Errno::ENOENT)
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        Ok(self.entries.get(index).cloned())
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

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        self.dir.readdir(index)
    }

    fn pnode(&self) -> Option<Arc<PathNode>> {
        Some(self.pnode.clone())
    }
}
