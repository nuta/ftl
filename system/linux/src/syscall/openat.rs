use alloc::sync::Arc;
use core::ffi::CStr;

use super::SyscallResult;
use crate::process::Process;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::O_CREAT;
use crate::types::sys::fcntl::O_DIRECTORY;
use crate::types::sys::fcntl::O_EXCL;
use crate::types::sys::fcntl::O_RDWR;
use crate::types::sys::fcntl::O_TRUNC;
use crate::types::sys::fcntl::O_WRONLY;
use crate::vfs::INode;
use crate::vfs::OpenedDir;
use crate::vfs::PathNode;

/// Looks up or creates a regular file at `path`.
fn lookup_or_create(
    process: &Process,
    path: &[u8],
    dirfd: Option<c_int>,
    exclusive: bool,
    mode: u32,
) -> Result<Arc<PathNode>, Errno> {
    let (parent, name) = process.lookup_parent(path, dirfd)?;
    if name.is_empty() || name == b"." || name == b".." {
        return Err(Errno::EISDIR);
    }

    let INode::Dir(dir) = parent.inode() else {
        return Err(Errno::ENOTDIR);
    };

    match dir.lookup(name) {
        Ok(inode) => {
            if exclusive {
                return Err(Errno::EEXIST);
            }

            return Ok(PathNode::new(parent, name, inode));
        }
        Err(Errno::ENOENT) => {}
        Err(error) => return Err(error),
    }

    // The file does not exist. Create it.
    let new_file = dir.create(name, mode)?;
    Ok(PathNode::new(parent, name, INode::File(new_file)))
}

pub fn sys_openat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    flags: c_int,
    mode: u32,
) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    let process = current.process();
    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let pnode = if flags & O_CREAT != 0 {
        let exclusive = flags & O_EXCL != 0;
        let mode = mode & 0o7777 & !process.umask();
        lookup_or_create(&process, path, dirfd, exclusive, mode)?
    } else {
        process.lookup_path(path, dirfd)?
    };

    let file = match pnode.inode() {
        INode::File(_) if flags & O_DIRECTORY != 0 => return Err(Errno::ENOTDIR),
        INode::File(file) => {
            if flags & O_TRUNC != 0 {
                file.truncate(0)?;
            }

            match file.open() {
                Ok(Some(opened)) => opened,
                Ok(None) => file.clone(),
                Err(errno) => return Err(errno),
            }
        }
        INode::Dir(_) if flags & (O_WRONLY | O_RDWR) != 0 => {
            // Directories are not writeable.
            return Err(Errno::EISDIR);
        }
        INode::Dir(_) if flags & O_CREAT != 0 => {
            // O_CREAT never opens a directory.
            return Err(Errno::EISDIR);
        }
        INode::Dir(dir) => Arc::new(OpenedDir::new(pnode.clone(), dir.clone())),
    };

    let fd = process.fd_table().lock().insert(file.clone(), flags)?;
    Ok(SyscallResult::Done(fd as c_long))
}
