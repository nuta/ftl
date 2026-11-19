use super::SyscallResult;
use super::newfstatat::sys_newfstatat;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;
use crate::types::sys::fcntl::AT_SYMLINK_NOFOLLOW;
use crate::types::sys::stat::Stat;

pub fn sys_lstat(current: &LxThread, path: *const u8, buf: *mut Stat) -> Result<SyscallResult, Errno> {
    sys_newfstatat(current, AT_FDCWD, path, buf, AT_SYMLINK_NOFOLLOW)
}
