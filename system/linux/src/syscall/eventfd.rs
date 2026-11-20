use super::SyscallResult;
use super::eventfd2::sys_eventfd2;
use crate::thread::Thread;
use crate::types::c_unsigned;
use crate::types::errno::Errno;

pub fn sys_eventfd(current: &Thread, initval: c_unsigned) -> Result<SyscallResult, Errno> {
    sys_eventfd2(current, initval, 0)
}
