use super::SyscallResult;
use super::unlinkat::sys_unlinkat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_unlink(current: &Thread, path: *const u8) -> Result<SyscallResult, Errno> {
    sys_unlinkat(current, AT_FDCWD, path, 0)
}
