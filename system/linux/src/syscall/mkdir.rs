use super::SyscallResult;
use super::mkdirat::sys_mkdirat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_mkdir(current: &Thread, path: *const u8) -> Result<SyscallResult, Errno> {
    sys_mkdirat(current, AT_FDCWD, path)
}
