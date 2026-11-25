use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_umask(current: &Thread, mask: u32) -> Result<SyscallResult, Errno> {
    let old = current.process().set_umask(mask);
    Ok(SyscallResult::Done(old as c_long))
}
