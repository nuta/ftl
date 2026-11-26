use super::SyscallResult;
use super::linkat::sys_linkat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_link(
    current: &Thread,
    old_path: *const u8,
    new_path: *const u8,
) -> Result<SyscallResult, Errno> {
    sys_linkat(current, AT_FDCWD, old_path, AT_FDCWD, new_path, 0)
}
