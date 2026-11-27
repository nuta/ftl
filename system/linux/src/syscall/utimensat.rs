use core::ffi::CStr;

use ftl::time::WallTime;
use ftl::time::WallTimeExt;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_EMPTY_PATH;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_NOFOLLOW;
use crate::types::sys::stat::UTIME_NOW;
use crate::types::sys::stat::UTIME_OMIT;
use crate::types::sys::time::TimeSpec;

/// Converts a timestamp. Returns `Ok(None)` on `UTIME_OMIT`.
fn to_wall_time(time: TimeSpec) -> Result<Option<WallTime>, Errno> {
    match time.tv_nsec {
        UTIME_NOW => Ok(Some(WallTime::now())),
        UTIME_OMIT => Ok(None),
        _ => {
            let nanos = time.to_nanos().ok_or(Errno::EINVAL)?;
            Ok(Some(WallTime::from_nanos(nanos)))
        }
    }
}

fn set_times(
    current: &Thread,
    fd: c_int,
    atime: Option<WallTime>,
    mtime: Option<WallTime>,
) -> Result<SyscallResult, Errno> {
    if fd == AT_FDCWD {
        return Err(Errno::EFAULT);
    }

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.file().set_times(atime, mtime)?;
    Ok(SyscallResult::Done(0))
}

pub fn sys_utimensat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    times: *const TimeSpec,
    flags: c_int,
) -> Result<SyscallResult, Errno> {
    if flags & !(AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW) != 0 {
        // Unsupported flags.
        return Err(Errno::EINVAL);
    }

    let (atime, mtime) = if times.is_null() {
        let now = WallTime::now();
        (Some(now), Some(now))
    } else {
        let atime = unsafe { times.read() }; // times[0]
        let mtime = unsafe { times.add(1).read() }; // times[1]
        (to_wall_time(atime)?, to_wall_time(mtime)?)
    };

    if atime.is_none() && mtime.is_none() {
        // Nothing to update.
        return Ok(SyscallResult::Done(0));
    }

    if path.is_null() {
        return set_times(current, dirfd, atime, mtime);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        if flags & AT_EMPTY_PATH == 0 {
            return Err(Errno::ENOENT);
        }

        if dirfd != AT_FDCWD {
            return set_times(current, dirfd, atime, mtime);
        }
    }

    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    let pnode = current.process().lookup_path(path, dirfd)?;
    pnode.inode().set_times(atime, mtime)?;
    Ok(SyscallResult::Done(0))
}
