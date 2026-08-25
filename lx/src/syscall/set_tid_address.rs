use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_set_tid_address(current: &Thread, tidptr: *mut c_int) -> Result<c_long, Errno> {
    // TODO: Support TID address
    let _ = tidptr;

    Ok(current.tid().as_int() as c_long)
}
