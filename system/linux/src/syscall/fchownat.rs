use core::ffi::CStr;

use super::SyscallResult;
use super::fchown::sys_fchown;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_EMPTY_PATH;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_NOFOLLOW;

/// Converts a user or group ID. `None` if it is -1.
pub fn convert_id(id: u32) -> Option<u32> {
    if id == u32::MAX { None } else { Some(id) }
}

pub fn sys_fchownat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    uid: u32,
    gid: u32,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if flags & !(AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW) != 0 {
        return Err(Errno::EINVAL);
    }

    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        if flags & AT_EMPTY_PATH == 0 {
            return Err(Errno::ENOENT);
        }

        if dirfd != AT_FDCWD {
            return sys_fchown(current, dirfd, uid, gid);
        }
    }

    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let pnode = current.process().lookup_path(path, dirfd)?;
    pnode.inode().chown(convert_id(uid), convert_id(gid))?;
    Ok(SyscallResult::Done(0))
}
