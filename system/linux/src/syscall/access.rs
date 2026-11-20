use super::SyscallResult;
use super::faccessat::sys_faccessat;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_access(current: &Thread, path: *const u8, mode: c_int) -> Result<SyscallResult, Errno> {
    sys_faccessat(current, AT_FDCWD, path, mode)
}
