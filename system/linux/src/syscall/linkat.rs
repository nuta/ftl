use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_FOLLOW;
use crate::vfs::INode;

pub fn sys_linkat(
    current: &Thread,
    old_dirfd: c_int,
    old_path: *const u8,
    new_dirfd: c_int,
    new_path: *const u8,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if old_path.is_null() || new_path.is_null() {
        return Err(Errno::EFAULT);
    }

    if flags & !AT_SYMLINK_FOLLOW != 0 {
        // Unsupported flags.
        return Err(Errno::EINVAL);
    }

    let old_path = unsafe { CStr::from_ptr(old_path.cast()) }.to_bytes();
    let new_path = unsafe { CStr::from_ptr(new_path.cast()) }.to_bytes();
    if old_path.is_empty() || new_path.is_empty() {
        return Err(Errno::ENOENT);
    }

    let old_dirfd = if old_dirfd == AT_FDCWD {
        None
    } else {
        Some(old_dirfd)
    };
    let new_dirfd = if new_dirfd == AT_FDCWD {
        None
    } else {
        Some(new_dirfd)
    };

    // Find the file to be linked.
    let process = current.process();
    let old_pnode = process.lookup_path(old_path, old_dirfd)?;
    let inode = old_pnode.inode().clone();

    // Directories can't be linked.
    if let INode::Dir(_) = inode {
        return Err(Errno::EPERM);
    }

    // Open the destination directory.
    let (new_parent, new_name) = process.lookup_parent(new_path, new_dirfd)?;
    if new_name.is_empty() || new_name == b"." || new_name == b".." {
        return Err(Errno::EEXIST);
    }

    let INode::Dir(new_dir) = new_parent.inode() else {
        return Err(Errno::ENOTDIR);
    };

    new_dir.link(new_name, inode)?;
    Ok(SyscallResult::Done(0))
}
