use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

use ftl::mutex::Mutex;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFREG;
use crate::types::sys::stat::Stat;
use crate::vfs::DirEntry;
use crate::vfs::DirOffset;
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

/// A list of directory entries, sorted by its ID.
struct EntryList<T> {
    /// Sorted by the ID (`usize`).
    items: Vec<(usize, T)>,
    next_id: usize,
}

impl<T> EntryList<T> {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 0,
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(_, item)| item)
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn try_push(&mut self, item: T) -> Result<(), Errno> {
        let id = self.next_id;
        if id == usize::MAX {
            return Err(Errno::ENOSPC);
        }

        // Reserve memory first to `push` without panicking.
        self.items.try_reserve(1).map_err(|_| Errno::ENOMEM)?;

        self.items.push((id, item));
        self.next_id += 1;
        Ok(())
    }

    /// Returns the first item at or after `offset`, and the next offset.
    fn readdir_from(&self, offset: DirOffset) -> Option<(&T, DirOffset)> {
        let index = self
            .items
            .partition_point(|(id, _)| *id < offset.as_usize());

        let (id, item) = self.items.get(index)?;
        let next_offset = DirOffset::new(id + 1);
        Some((item, next_offset))
    }

    /// Removes the first item that matches `pred`.
    fn remove<P>(&mut self, pred: P) -> Option<T>
    where
        P: Fn(&T) -> bool,
    {
        let index = self.items.iter().position(|(_, item)| pred(item))?;
        let (_offset, item) = self.items.remove(index);
        Some(item)
    }
}

pub struct TmpDir {
    entries: Mutex<EntryList<DirEntry>>,
}

impl TmpDir {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(EntryList::new()),
        }
    }

    pub fn add(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        let mut entries = self.entries.lock();
        for entry in entries.iter() {
            if *entry.name == *name {
                return Err(Errno::EEXIST);
            }
        }

        entries.try_push(DirEntry::new(name, inode))
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

    fn readdir(&self, offset: DirOffset) -> Result<Option<(DirEntry, DirOffset)>, Errno> {
        let entries = self.entries.lock();
        let Some((entry, next_offset)) = entries.readdir_from(offset) else {
            return Ok(None);
        };

        Ok(Some((entry.clone(), next_offset)))
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

    fn is_empty(&self) -> Result<bool, Errno> {
        let entries = self.entries.lock();
        Ok(entries.is_empty())
    }

    fn link(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        self.add(name, inode)
    }

    fn unlink(&self, name: &[u8]) -> Result<(), Errno> {
        let mut entries = self.entries.lock();
        let Some(entry) = entries.iter().find(|entry| *entry.name == *name) else {
            return Err(Errno::ENOENT);
        };

        if let INode::Dir(_) = entry.inode {
            return Err(Errno::EISDIR);
        }

        entries.remove(|entry| *entry.name == *name);
        Ok(())
    }

    fn rmdir(&self, name: &[u8]) -> Result<(), Errno> {
        let mut entries = self.entries.lock();
        let Some(entry) = entries.iter().find(|entry| *entry.name == *name) else {
            return Err(Errno::ENOENT);
        };

        let INode::Dir(dir) = &entry.inode else {
            return Err(Errno::ENOTDIR);
        };

        if !dir.is_empty()? {
            return Err(Errno::ENOTEMPTY);
        }

        entries.remove(|entry| *entry.name == *name);
        Ok(())
    }
}
