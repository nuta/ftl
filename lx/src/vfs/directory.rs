use alloc::sync::Arc;

use crate::types::errno::Errno;
use crate::vfs::FileLike;
use crate::vfs::INode;
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

pub struct StaticDir<const N: usize> {
    entries: [DirEntry; N],
}

impl<const N: usize> StaticDir<N> {
    pub fn new(entries: [DirEntry; N]) -> Self {
        Self { entries }
    }
}

impl<const N: usize> Directory for StaticDir<N> {
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
    dir: Arc<dyn Directory>,
}

impl OpenedDir {
    pub fn new(dir: Arc<dyn Directory>) -> Self {
        Self { dir }
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
}
