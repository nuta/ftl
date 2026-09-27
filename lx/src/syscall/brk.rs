use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_brk(current: &LxThread, addr: usize) -> Result<SyscallResult, Errno> {
    Ok(SyscallResult::Done(current.vm().brk(addr) as c_long))
}
