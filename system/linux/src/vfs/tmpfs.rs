use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

use ftl::mutex::Mutex;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFREG;
use crate::types::sys::stat::Stat;
use crate::vfs::DirEntry;
use crate::vfs::Directory;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::addr_as_ino;
use crate::wait_queue::Sleep;

pub struct TmpFile {
    data: Mutex<Vec<u8>>,
}

impl TmpFile {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }
}

impl FileLike for TmpFile {
    fn read(
        &self,
        buf: &mut [u8],
        offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }

        let copy_len = min(buf.len(), data.len() - offset);
        buf[..copy_len].copy_from_slice(&data[offset..offset + copy_len]);
        Ok(copy_len)
    }

    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let end = offset.checked_add(buf.len()).ok_or(Errno::EINVAL)?;
        let mut data = self.data.lock();

        // Extend the buffer if it is not large enough.
        let len = data.len();
        if len < end {
            data.try_reserve(end - len).map_err(|_| Errno::ENOMEM)?;
            data.resize(end, 0);
        }

        data[offset..end].copy_from_slice(buf);
        Ok(buf.len())
    }

    fn truncate(&self, len: usize) -> Result<(), Errno> {
        let mut data = self.data.lock();

        // Reserve memory first to `resize` without panicking.
        let old_len = data.len();
        if old_len < len {
            data.try_reserve(len - old_len).map_err(|_| Errno::ENOMEM)?;
        }

        data.resize(len, 0);
        Ok(())
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        Ok(self.data.lock().len())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        let size = self.data.lock().len();
        Ok(Stat::new(addr_as_ino(self), S_IFREG | 0o755, size))
    }
}

pub struct TmpDir {
    entries: Mutex<Vec<DirEntry>>,
}

impl TmpDir {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn add(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        let mut entries = self.entries.lock();
        if entries.iter().any(|entry| *entry.name == *name) {
            return Err(Errno::EEXIST);
        }

        entries.push(DirEntry::new(name, inode));
        Ok(())
    }
}

impl Directory for TmpDir {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        let entries = self.entries.lock();
        for entry in entries.iter() {
            if *entry.name == *name {
                return Ok(entry.inode.clone());
            }
        }

        Err(Errno::ENOENT)
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        Ok(self.entries.lock().get(index).cloned())
    }

    fn create(&self, name: &[u8]) -> Result<Arc<dyn FileLike>, Errno> {
        let new_file = Arc::new(TmpFile::new());
        self.add(name, INode::File(new_file.clone()))?;
        Ok(new_file)
    }

    fn mkdir(&self, name: &[u8]) -> Result<(), Errno> {
        let new_dir = TmpDir::new();
        self.add(name, INode::Dir(Arc::new(new_dir)))
    }
}
