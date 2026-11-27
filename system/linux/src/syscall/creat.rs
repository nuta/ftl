use super::SyscallResult;
use super::openat::sys_openat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::O_CREAT;
use crate::types::sys::fcntl::O_TRUNC;
use crate::types::sys::fcntl::O_WRONLY;

pub fn sys_creat(current: &Thread, path: *const u8, mode: u32) -> Result<SyscallResult, Errno> {
    sys_openat(current, AT_FDCWD, path, O_CREAT | O_WRONLY | O_TRUNC, mode)
}
