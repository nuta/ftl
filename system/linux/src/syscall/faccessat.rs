use super::SyscallResult;
use super::faccessat2::sys_faccessat2;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_faccessat(
    current: &Thread,
    dirfd: c_int,
    path: *const u8,
    mode: c_int,
) -> Result<SyscallResult, Errno> {
    sys_faccessat2(current, dirfd, path, mode, 0)
}
