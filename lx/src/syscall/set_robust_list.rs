use ftl::trace;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_set_robust_list(
    _current: &LxThread,
    _head: *const c_void,
    _len: size_t,
) -> Result<SyscallResult, Errno> {
    trace!("set_robust_list is not implemented");
    Ok(SyscallResult::Done(0))
}
