use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_getgroups(_current: &Thread, _size: c_int) -> Result<SyscallResult, Errno> {
    // TODO:
    Ok(SyscallResult::Done(0))
}
