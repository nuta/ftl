use super::accept4::sys_accept4;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_accept(
    current: &LxThread,
    fd: c_int,
    addr: *mut u8,
    addr_len: *mut u32,
) -> Result<c_long, Errno> {
    sys_accept4(current, fd, addr, addr_len, 0)
}
