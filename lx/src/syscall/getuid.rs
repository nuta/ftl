use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::errno::Errno;

pub fn sys_getuid(_current: &LxThread) -> Result<SyscallResult, Errno> {
    // You're always root, with great power comes great responsibility.
    Ok(SyscallResult::Done(0))
}
