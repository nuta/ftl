use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_getpid(current: &LxThread) -> Result<c_long, Errno> {
    Ok(current.process().id().as_int() as c_long)
}
