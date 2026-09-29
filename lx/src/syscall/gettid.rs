use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_gettid(current: &LxThread) -> Result<SyscallResult, Errno> {
    Ok(SyscallResult::Done(current.tid().as_int() as c_long))
}
