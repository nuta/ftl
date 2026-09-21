use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::open_file::OpenFile;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::vfs::FileLike;

#[derive(Clone)]
struct Entry {
    file: Arc<OpenFile>,
    cloexec: bool,
}

#[derive(Clone)]
pub struct FdTable {
    open_files: Vec<Option<Entry>>,
    active_fds: usize,
    capacity: usize,
}

impl FdTable {
    pub fn new(capacity: usize) -> Self {
        Self {
            open_files: Vec::new(),
            active_fds: 0,
            capacity,
        }
    }

    pub fn insert(&mut self, file: Arc<dyn FileLike>, flags: c_int) -> Result<c_int, Errno> {
        if self.active_fds >= self.capacity {
            return Err(Errno::EMFILE);
        }

        for fd in 0..self.capacity {
            if fd >= self.open_files.len() || self.open_files[fd].is_none() {
                self.insert_at(fd as c_int, file, flags)?;
                return Ok(fd as c_int);
            }
        }

        Err(Errno::EMFILE)
    }

    /// Inserts two files.
    pub fn insert2(
        &mut self,
        file1: Arc<dyn FileLike>,
        flags1: c_int,
        file2: Arc<dyn FileLike>,
        flags2: c_int,
    ) -> Result<(c_int, c_int), Errno> {
        let fd1 = self.insert(file1, flags1)?;
        let fd2 = match self.insert(file2, flags2) {
            Ok(fd) => fd,
            Err(error) => {
                let _ = self.remove(fd1);
                return Err(error);
            }
        };

        Ok((fd1, fd2))
    }

    pub fn insert_at(
        &mut self,
        fd: c_int,
        file: Arc<dyn FileLike>,
        flags: c_int,
    ) -> Result<(), Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let fd = fd as usize;
        if fd >= self.capacity {
            return Err(Errno::EMFILE);
        }

        if fd >= self.open_files.len() {
            self.open_files.resize(fd + 1, None);
        }

        let new = Entry {
            file: Arc::new(OpenFile::new(file, flags)),
            cloexec: flags & O_CLOEXEC != 0,
        };

        let old = self.open_files[fd].replace(new);
        if old.is_none() {
            self.active_fds += 1;
        }

        Ok(())
    }

    pub fn get(&self, fd: c_int) -> Result<&Arc<OpenFile>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let slot = self.open_files.get(fd as usize);
        match slot {
            Some(Some(entry)) => Ok(&entry.file),
            _ => Err(Errno::EBADF),
        }
    }

    /// Returns if the fd is marked as close-on-exec.
    pub fn get_cloexec(&self, fd: c_int) -> Result<bool, Errno> {
        let slot = self.open_files.get(fd as usize);
        match slot {
            Some(Some(entry)) => Ok(entry.cloexec),
            _ => Err(Errno::EBADF),
        }
    }

    /// Updates the close-on-exec flag for the fd.
    pub fn set_cloexec(&mut self, fd: c_int, cloexec: bool) -> Result<(), Errno> {
        let slot = self.open_files.get_mut(fd as usize);
        match slot {
            Some(Some(entry)) => {
                entry.cloexec = cloexec;
                Ok(())
            }
            _ => Err(Errno::EBADF),
        }
    }

    fn find_free_fd(&self, minfd: c_int) -> Result<usize, Errno> {
        if minfd < 0 {
            return Err(Errno::EINVAL);
        }

        for fd in (minfd as usize)..self.capacity {
            if fd >= self.open_files.len() || self.open_files[fd].is_none() {
                return Ok(fd);
            }
        }

        Err(Errno::EMFILE)
    }

    /// Duplicates `oldfd` to `newfd`.
    fn do_dup(&mut self, oldfd: c_int, newfd: usize, cloexec: bool) -> Result<c_int, Errno> {
        let file = self.get(oldfd)?.clone();
        if newfd >= self.open_files.len() {
            self.open_files.resize(newfd + 1, None);
        }

        let old = self.open_files[newfd].replace(Entry { file, cloexec });
        if old.is_none() {
            self.active_fds += 1;
        }

        Ok(newfd as c_int)
    }

    pub fn dup(&mut self, oldfd: c_int, minfd: c_int, cloexec: bool) -> Result<c_int, Errno> {
        let newfd = self.find_free_fd(minfd)?;
        self.do_dup(oldfd, newfd, cloexec)
    }

    pub fn dup2(&mut self, oldfd: c_int, newfd: c_int) -> Result<c_int, Errno> {
        // > If oldfd is a valid file descriptor, and newfd has the same
        // > value as oldfd, then dup2() does nothing, and returns newfd.
        // >
        // > https://man7.org/linux/man-pages/man2/dup.2.html
        if oldfd == newfd {
            // Check the validity of oldfd.
            self.get(oldfd)?;
            return Ok(newfd);
        }

        self.dup3(oldfd, newfd, 0)
    }

    pub fn dup3(&mut self, oldfd: c_int, newfd: c_int, flags: c_int) -> Result<c_int, Errno> {
        if flags & !O_CLOEXEC != 0 {
            // Reject unsupported flags.
            return Err(Errno::EINVAL);
        }

        if oldfd == newfd {
            // Unlike dup2:
            //
            // > If oldfd equals newfd, then dup3() fails with the error EINVAL.
            // >
            // > https://man7.org/linux/man-pages/man2/dup.2.html
            return Err(Errno::EINVAL);
        }

        if newfd < 0 || (newfd as usize) >= self.capacity {
            return Err(Errno::EBADF);
        }

        self.do_dup(oldfd, newfd as usize, flags & O_CLOEXEC != 0)
    }

    pub fn remove(&mut self, fd: c_int) -> Result<Arc<OpenFile>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let slot = self.open_files.get_mut(fd as usize);
        let entry = match slot {
            Some(entry) => entry.take().ok_or(Errno::EBADF)?,
            _ => return Err(Errno::EBADF),
        };

        self.active_fds -= 1;
        Ok(entry.file)
    }

    /// Closes file descriptors that are marked close-on-exec.
    pub fn close_on_exec(&mut self) {
        for slot in &mut self.open_files {
            if let Some(entry) = slot.as_ref() {
                if entry.cloexec {
                    *slot = None;
                    self.active_fds -= 1;
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.active_fds = 0;
        self.open_files.clear();
    }
}
