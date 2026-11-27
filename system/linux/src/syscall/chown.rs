use super::SyscallResult;
use super::fchownat::sys_fchownat;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_FDCWD;

pub fn sys_chown(
    current: &Thread,
    path: *const u8,
    uid: u32,
    gid: u32,
) -> Result<SyscallResult, Errno> {
    sys_fchownat(current, AT_FDCWD, path, uid, gid, 0)
}
