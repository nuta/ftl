use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;

pub fn sys_getuid(_current: &Thread) -> Result<SyscallResult, Errno> {
    // You're always root, with great power comes great responsibility.
    Ok(SyscallResult::Done(0))
}
