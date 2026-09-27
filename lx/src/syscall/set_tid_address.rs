use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_set_tid_address(current: &LxThread, tidptr: *mut c_int) -> Result<SyscallResult, Errno> {
    // TODO: Support TID address
    let _ = tidptr;

    Ok(SyscallResult::Done(current.tid().as_int() as c_long))
}
