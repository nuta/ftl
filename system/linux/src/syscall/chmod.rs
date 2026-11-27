use super::SyscallResult;
use super::fchmodat::sys_fchmodat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_chmod(current: &Thread, path: *const u8, mode: u32) -> Result<SyscallResult, Errno> {
    sys_fchmodat(current, AT_FDCWD, path, mode)
}
