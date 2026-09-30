use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_set_tid_address(current: &LxThread, tidptr: *mut c_int) -> Result<SyscallResult, Errno> {
    let uaddr = if tidptr.is_null() {
        None
    } else {
        Some(tidptr.addr())
    };

    current.set_clear_child_tid(uaddr);
    Ok(SyscallResult::Done(current.tid().as_int() as c_long))
}
