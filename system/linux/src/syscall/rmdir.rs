use super::SyscallResult;
use super::unlinkat::sys_unlinkat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_REMOVEDIR;

pub fn sys_rmdir(current: &Thread, path: *const u8) -> Result<SyscallResult, Errno> {
    sys_unlinkat(current, AT_FDCWD, path, AT_REMOVEDIR)
}
