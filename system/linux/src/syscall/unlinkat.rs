use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_REMOVEDIR;
use crate::vfs::INode;

pub fn sys_unlinkat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    if flags & !AT_REMOVEDIR != 0 {
        // Unsupported flags.
        return Err(Errno::EINVAL);
    }

    let mut path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    let rmdir = flags & AT_REMOVEDIR != 0;
    if rmdir {
        // Remove trailing slashes when deleting a directory.
        while path.len() > 1 && path.ends_with(b"/") {
            path = &path[..path.len() - 1];
        }
    }

    // Open the parent directory.
    let process = current.process();
    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let (parent, name) = process.lookup_parent(path, dirfd)?;
    if name.is_empty() || name == b"." || name == b".." {
        let errno = if rmdir { Errno::EINVAL } else { Errno::EISDIR };
        return Err(errno);
    }

    let INode::Dir(dir) = parent.inode() else {
        return Err(Errno::ENOTDIR);
    };

    if rmdir {
        dir.rmdir(name)?;
    } else {
        dir.unlink(name)?;
    }

    Ok(SyscallResult::Done(0))
}
