use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::vfs::INode;

pub fn sys_mkdirat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    mode: u32,
) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let mut path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    // Remove trailing slashes.
    while path.len() > 1 && path.ends_with(b"/") {
        path = &path[..path.len() - 1];
    }

    let process = current.process();

    // Find the parent directory.
    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let (parent, name) = process.lookup_parent(path, dirfd)?;

    // Reject invalid directory names.
    if name.is_empty() || name == b"." || name == b".." {
        return Err(Errno::EEXIST);
    }

    let INode::Dir(dir) = parent.inode() else {
        return Err(Errno::ENOTDIR);
    };

    // 0o1000 = sticky bit
    let mode = mode & 0o1777 & !process.umask();

    dir.mkdir(name, mode)?;
    Ok(SyscallResult::Done(0))
}
