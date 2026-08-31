use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_fork(current: &LxThread, syscall_sp: usize) -> Result<c_long, Errno> {
    let pid = current.process().fork(current, syscall_sp)?;
    Ok(pid.as_int() as c_long)
}
