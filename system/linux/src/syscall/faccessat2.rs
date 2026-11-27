use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_EACCESS;
use crate::types::sys::fcntl::AT_EMPTY_PATH;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_NOFOLLOW;
use crate::types::sys::stat::Stat;

const F_OK: c_int = 0;
const R_OK: c_int = 4;
const W_OK: c_int = 2;
const X_OK: c_int = 1;

/// Checks if the current user can access the file in `mode`.
fn check_access(stat: Stat, mode: c_int) -> Result<SyscallResult, Errno> {
    // We don't support users. Handle as the owner.
    let perm = stat.st_mode >> 6;

    let requested = (mode as u32) & 0o7;
    if (perm & requested) != requested {
        return Err(Errno::EACCES);
    }

    Ok(SyscallResult::Done(0))
}

pub fn sys_faccessat2(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    mode: c_int,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if mode & !(F_OK | R_OK | W_OK | X_OK) != 0 {
        return Err(Errno::EINVAL);
    }

    if flags & !(AT_EACCESS | AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW) != 0 {
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
            let file = {
                let process = current.process();
                let fd_table = process.fd_table().lock();
                fd_table.get(dirfd)?.clone()
            };

            let stat = file.file().stat()?;
            return check_access(stat, mode);
        }
    }

    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let pnode = current.process().lookup_path(path, dirfd)?;
    let stat = pnode.inode().stat()?;
    check_access(stat, mode)
}
