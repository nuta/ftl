use super::SyscallResult;
use super::openat::sys_openat;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_open(current: &LxThread, path: *const u8, flags: c_int) -> Result<SyscallResult, Errno> {
    sys_openat(current, AT_FDCWD, path, flags)
}
