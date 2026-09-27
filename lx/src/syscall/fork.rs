use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_fork(current: &LxThread, frame: &mut SyscallFrame) -> Result<SyscallResult, Errno> {
    let pid = current.process().fork(current, frame)?;
    Ok(SyscallResult::Done(pid.as_int() as c_long))
}
