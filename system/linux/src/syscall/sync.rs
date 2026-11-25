use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;

pub fn sys_sync(_current: &Thread) -> Result<SyscallResult, Errno> {
    // TODO:
    Ok(SyscallResult::Done(0))
}
