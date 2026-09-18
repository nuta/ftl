use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_fork(current: &LxThread, frame: &mut SyscallFrame) -> Result<c_long, Errno> {
    let pid = current.process().fork(current, frame)?;
    Ok(pid.as_int() as c_long)
}
