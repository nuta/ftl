use super::SyscallResult;
use super::eventfd2::sys_eventfd2;
use crate::thread::LxThread;
use crate::types::c_unsigned;
use crate::types::errno::Errno;

pub fn sys_eventfd(current: &LxThread, initval: c_unsigned) -> Result<SyscallResult, Errno> {
    sys_eventfd2(current, initval, 0)
}
