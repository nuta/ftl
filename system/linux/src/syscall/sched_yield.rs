use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;

pub fn sys_sched_yield(_current: &Thread) -> Result<SyscallResult, Errno> {
    ftl::thread::thread_yield();
    Ok(SyscallResult::Done(0))
}
