use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_getpriority(
    _current: &Thread,
    _which: c_int,
    _who: c_int,
) -> Result<SyscallResult, Errno> {
    // Hardcoded to 20 (default priority).
    Ok(SyscallResult::Done(20))
}
