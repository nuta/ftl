use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_brk(current: &LxThread, addr: usize) -> Result<c_long, Errno> {
    Ok(current.process().brk(addr) as c_long)
}
