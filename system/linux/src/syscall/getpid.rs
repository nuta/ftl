use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_getpid(current: &Thread) -> Result<SyscallResult, Errno> {
    Ok(SyscallResult::Done(
        current.process().id().as_int() as c_long
    ))
}
