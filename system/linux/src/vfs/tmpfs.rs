use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

use ftl::mutex::Mutex;
use ftl::time::WallTime;
use ftl::time::WallTimeExt;

use crate::types::errno::Errno;
use crate::types::sys::stat::S_IFDIR;
use crate::types::sys::stat::S_IFREG;
use crate::types::sys::stat::Stat;
use crate::types::sys::time::TimeSpec;
use crate::vfs::DirEntry;
use crate::vfs::DirOffset;
use crate::vfs::Directory;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::addr_as_ino;
use crate::wait_queue::Sleep;

struct Times {
    atime: WallTime,
    mtime: WallTime,
    ctime: WallTime,
}

impl Times {
    fn now() -> Self {
        let now = WallTime::now();
        Self {
            atime: now,
            mtime: now,
            ctime: now,
        }
    }

    fn update_mtime(&mut self) {
        let now = WallTime::now();
        self.mtime = now;
        self.ctime = now;
    }

    fn update_ctime(&mut self) {
        self.ctime = WallTime::now();
    }

    fn set(&mut self, atime: Option<WallTime>, mtime: Option<WallTime>) {
        if let Some(atime) = atime {
            self.atime = atime;
        }

        if let Some(mtime) = mtime {
            self.mtime = mtime;
        }

        self.update_ctime();
    }

    fn fill_stat(&self, stat: &mut Stat) {
        let atime = TimeSpec::from_nanos(self.atime.as_nanos());
        let mtime = TimeSpec::from_nanos(self.mtime.as_nanos());
        let ctime = TimeSpec::from_nanos(self.ctime.as_nanos());
        stat.st_atime = atime.tv_sec as i64;
        stat.st_atime_nsec = atime.tv_nsec as i64;
        stat.st_mtime = mtime.tv_sec as i64;
        stat.st_mtime_nsec = mtime.tv_nsec as i64;
        stat.st_ctime = ctime.tv_sec as i64;
        stat.st_ctime_nsec = ctime.tv_nsec as i64;
    }
}

struct FileInner {
    data: Vec<u8>,
    mode: u32,
    uid: u32,
    gid: u32,
    times: Times,
}

pub struct TmpFile {
    inner: Mutex<FileInner>,
}

impl TmpFile {
    pub fn new(mode: u32) -> Self {
        Self {
            inner: Mutex::new(FileInner {
                data: Vec::new(),
                mode,
                uid: 0,
                gid: 0,
                times: Times::now(),
            }),
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
        let inner = self.inner.lock();
        if offset >= inner.data.len() {
            return Ok(0);
        }

        let copy_len = min(buf.len(), inner.data.len() - offset);
        buf[..copy_len].copy_from_slice(&inner.data[offset..offset + copy_len]);
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
        let mut inner = self.inner.lock();

        // Extend the buffer if it is not large enough.
        let len = inner.data.len();
        if len < end {
            inner
                .data
                .try_reserve(end - len)
                .map_err(|_| Errno::ENOMEM)?;
            inner.data.resize(end, 0);
        }

        inner.data[offset..end].copy_from_slice(buf);
        inner.times.update_mtime();
        Ok(buf.len())
    }

    fn truncate(&self, len: usize) -> Result<(), Errno> {
        let mut inner = self.inner.lock();

        // Reserve memory first to `resize` without panicking.
        let old_len = inner.data.len();
        if old_len < len {
            inner
                .data
                .try_reserve(len - old_len)
                .map_err(|_| Errno::ENOMEM)?;
        }

        inner.data.resize(len, 0);
        inner.times.update_mtime();
        Ok(())
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn size(&self) -> Result<usize, Errno> {
        let inner = self.inner.lock();
        Ok(inner.data.len())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        let inner = self.inner.lock();
        let size = inner.data.len();
        let mut stat = Stat::new(addr_as_ino(self), S_IFREG | inner.mode, size);
        stat.st_uid = inner.uid;
        stat.st_gid = inner.gid;
        inner.times.fill_stat(&mut stat);
        Ok(stat)
    }

    fn chmod(&self, mode: u32) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        inner.mode = mode & 0o7777;
        inner.times.update_ctime();
        Ok(())
    }

    fn set_times(&self, atime: Option<WallTime>, mtime: Option<WallTime>) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        inner.times.set(atime, mtime);
        Ok(())
    }

    fn chown(&self, uid: Option<u32>, gid: Option<u32>) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        if let Some(uid) = uid {
            inner.uid = uid;
        }

        if let Some(gid) = gid {
            inner.gid = gid;
        }

        inner.times.update_ctime();
        Ok(())
    }

    fn fsync(&self) -> Result<(), Errno> {
        Ok(())
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

struct DirInner {
    entries: EntryList<DirEntry>,
    mode: u32,
    uid: u32,
    gid: u32,
    times: Times,
}

pub struct TmpDir {
    inner: Mutex<DirInner>,
}

impl TmpDir {
    pub fn new(mode: u32) -> Self {
        Self {
            inner: Mutex::new(DirInner {
                entries: EntryList::new(),
                mode,
                uid: 0,
                gid: 0,
                times: Times::now(),
            }),
        }
    }

    pub fn add(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        for entry in inner.entries.iter() {
            if *entry.name == *name {
                return Err(Errno::EEXIST);
            }
        }

        inner.entries.try_push(DirEntry::new(name, inode))?;
        inner.times.update_mtime();
        Ok(())
    }
}

impl Directory for TmpDir {
    fn lookup(&self, name: &[u8]) -> Result<INode, Errno> {
        let inner = self.inner.lock();
        for entry in inner.entries.iter() {
            if *entry.name == *name {
                return Ok(entry.inode.clone());
            }
        }

        Err(Errno::ENOENT)
    }

    fn readdir(&self, offset: DirOffset) -> Result<Option<(DirEntry, DirOffset)>, Errno> {
        let inner = self.inner.lock();
        let Some((entry, next_offset)) = inner.entries.readdir_from(offset) else {
            return Ok(None);
        };

        Ok(Some((entry.clone(), next_offset)))
    }

    fn create(&self, name: &[u8], mode: u32) -> Result<Arc<dyn FileLike>, Errno> {
        let new_file = Arc::new(TmpFile::new(mode));
        self.add(name, INode::File(new_file.clone()))?;
        Ok(new_file)
    }

    fn mkdir(&self, name: &[u8], mode: u32) -> Result<(), Errno> {
        let new_dir = TmpDir::new(mode);
        self.add(name, INode::Dir(Arc::new(new_dir)))
    }

    fn is_empty(&self) -> Result<bool, Errno> {
        let inner = self.inner.lock();
        Ok(inner.entries.is_empty())
    }

    fn link(&self, name: &[u8], inode: INode) -> Result<(), Errno> {
        self.add(name, inode)
    }

    fn unlink(&self, name: &[u8]) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        let Some(entry) = inner.entries.iter().find(|entry| *entry.name == *name) else {
            return Err(Errno::ENOENT);
        };

        if let INode::Dir(_) = entry.inode {
            return Err(Errno::EISDIR);
        }

        inner.entries.remove(|entry| *entry.name == *name);
        inner.times.update_mtime();
        Ok(())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        let inner = self.inner.lock();
        let mut stat = Stat::new(addr_as_ino(self), S_IFDIR | inner.mode, 0);
        stat.st_uid = inner.uid;
        stat.st_gid = inner.gid;
        inner.times.fill_stat(&mut stat);
        Ok(stat)
    }

    fn chmod(&self, mode: u32) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        inner.mode = mode & 0o7777;
        inner.times.update_ctime();
        Ok(())
    }

    fn set_times(&self, atime: Option<WallTime>, mtime: Option<WallTime>) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        inner.times.set(atime, mtime);
        Ok(())
    }

    fn chown(&self, uid: Option<u32>, gid: Option<u32>) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        if let Some(uid) = uid {
            inner.uid = uid;
        }

        if let Some(gid) = gid {
            inner.gid = gid;
        }

        inner.times.update_ctime();
        Ok(())
    }

    fn fsync(&self) -> Result<(), Errno> {
        Ok(())
    }

    fn rmdir(&self, name: &[u8]) -> Result<(), Errno> {
        let mut inner = self.inner.lock();
        let Some(entry) = inner.entries.iter().find(|entry| *entry.name == *name) else {
            return Err(Errno::ENOENT);
        };

        let INode::Dir(dir) = &entry.inode else {
            return Err(Errno::ENOTDIR);
        };

        if !dir.is_empty()? {
            return Err(Errno::ENOTEMPTY);
        }

        inner.entries.remove(|entry| *entry.name == *name);
        inner.times.update_mtime();
        Ok(())
    }
}
