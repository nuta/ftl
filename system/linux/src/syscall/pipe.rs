use super::SyscallResult;
use super::pipe2::sys_pipe2;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_pipe(current: &Thread, pipefd: *mut c_int) -> Result<SyscallResult, Errno> {
    sys_pipe2(current, pipefd, 0)
}
