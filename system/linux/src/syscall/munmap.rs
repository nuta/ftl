use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;

pub fn sys_munmap(current: &Thread, addr: usize, len: usize) -> Result<SyscallResult, Errno> {
    current.vm().munmap(addr, len)?;
    Ok(SyscallResult::Done(0))
}
