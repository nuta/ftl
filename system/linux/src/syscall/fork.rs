use core::ptr;

use super::SyscallResult;
use super::clone::sys_clone;
use crate::arch::SyscallFrame;
use crate::thread::Thread;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::signal::SIGCHLD;

pub fn sys_fork(current: &Thread, frame: &SyscallFrame) -> Result<SyscallResult, Errno> {
    sys_clone(
        current,
        frame,
        SIGCHLD as c_ulong,
        0,
        ptr::null_mut(),
        ptr::null_mut(),
        0,
    )
}
