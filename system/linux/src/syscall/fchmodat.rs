use core::ffi::CStr;

use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_fchmodat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    mode: u32,
) -> Result<SyscallResult, Errno> {
    if path.is_null() {
        return Err(Errno::EFAULT);
    }

    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();
    if path.is_empty() {
        return Err(Errno::ENOENT);
    }

    let dirfd = if dirfd == AT_FDCWD { None } else { Some(dirfd) };

    let pnode = current.process().lookup_path(path, dirfd)?;
    pnode.inode().chmod(mode)?;
    Ok(SyscallResult::Done(0))
}
