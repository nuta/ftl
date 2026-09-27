use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_dup2(current: &LxThread, oldfd: c_int, newfd: c_int) -> Result<SyscallResult, Errno> {
    let fd = current.process().fd_table().lock().dup2(oldfd, newfd)?;
    Ok(SyscallResult::Done(fd as c_long))
}
