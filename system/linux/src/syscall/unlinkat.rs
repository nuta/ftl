use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
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

    // TODO: Support AT_REMOVEDIR.
    if flags != 0 {
        return Err(Errno::EINVAL);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    // Open the parent directory.
    let process = current.process();
    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let (parent, name) = process.lookup_parent(path, dirfd)?;
    if name.is_empty() || name == b"." || name == b".." {
        return Err(Errno::EISDIR);
    }

    let INode::Dir(dir) = parent.inode() else {
        return Err(Errno::ENOTDIR);
    };

    let inode = dir.lookup(name)?;

    if let INode::Dir(_) = inode {
        return Err(Errno::EISDIR);
    }

    dir.remove(name)?;
    Ok(SyscallResult::Done(0))
}
