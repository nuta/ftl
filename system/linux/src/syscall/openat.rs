use alloc::sync::Arc;
use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::O_DIRECTORY;
use crate::types::sys::fcntl::O_RDWR;
use crate::types::sys::fcntl::O_WRONLY;
use crate::vfs::INode;
use crate::vfs::OpenedDir;

pub fn sys_openat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    flags: c_int,
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
    let pnode = process.lookup_path(path, dirfd)?;

    let file = match pnode.inode() {
        INode::File(_) if flags & O_DIRECTORY != 0 => return Err(Errno::ENOTDIR),
        INode::File(file) => {
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
        INode::Dir(dir) => Arc::new(OpenedDir::new(pnode.clone(), dir.clone())),
    };

    let fd = process.fd_table().lock().insert(file.clone(), flags)?;
    Ok(SyscallResult::Done(fd as c_long))
}
