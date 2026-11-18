use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

const F_OK: c_int = 0;
const R_OK: c_int = 4;
const W_OK: c_int = 2;
const X_OK: c_int = 1;

pub fn sys_faccessat(
    current: &LxThread,
    dirfd: c_int,
    path: *const u8,
    mode: c_int,
) -> Result<SyscallResult, Errno> {
    if mode & !(F_OK | R_OK | W_OK | X_OK) != 0 {
        // Unsupported mode bits.
        return Err(Errno::EINVAL);
    }

    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    // Resolve the path to see if it exists.
    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };
    current.process().lookup_path(path, dirfd)?;

    Ok(SyscallResult::Done(0))
}
