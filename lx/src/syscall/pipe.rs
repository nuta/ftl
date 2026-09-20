use super::pipe2::sys_pipe2;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_pipe(current: &LxThread, pipefd: *mut c_int) -> Result<c_long, Errno> {
    sys_pipe2(current, pipefd, 0)
}
