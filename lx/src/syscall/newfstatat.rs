use core::ffi::CStr;

use super::SyscallResult;
use super::fstat::sys_fstat;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_EMPTY_PATH;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_NOFOLLOW;
use crate::types::sys::stat::Stat;

pub fn sys_newfstatat(
    current: &LxThread,
    dirfd: c_int,
    path: *const u8,
    buf: *mut Stat,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if flags & !(AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW) != 0 {
        // Unsupported flags.
        return Err(Errno::EINVAL);
    }

    if path.is_null() || buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        if flags & AT_EMPTY_PATH == 0 {
            return Err(Errno::ENOENT);
        }

        if dirfd != AT_FDCWD {
            // If the path is empty and dirfd != AT_FDCWD, it is treated as a
            // fsstat of the dirfd.
            return sys_fstat(current, dirfd, buf);
        }
    }

    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let stat = current.process().lookup_path(path, dirfd)?.inode().stat()?;

    unsafe { buf.write(stat) };
    Ok(SyscallResult::Done(0))
}
