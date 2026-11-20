use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_madvise(
    _current: &Thread,
    _addr: usize,
    _len: usize,
    _advice: c_int,
) -> Result<SyscallResult, Errno> {
    // TODO: Implement properly, especially MADV_DONTNEED.
    Ok(SyscallResult::Done(0))
}
