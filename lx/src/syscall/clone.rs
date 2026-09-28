use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::errno::Errno;

pub fn sys_clone(
    current: &LxThread,
    frame: &mut SyscallFrame,
    flags: c_ulong,
    stack: usize,
    _parent_tid: *mut c_int,
    _child_tid: *mut c_int,
    _tls: c_ulong,
) -> Result<SyscallResult, Errno> {
    if stack != 0 {
        // TODO: Support a new stack for the child.
        return Err(Errno::EINVAL);
    }

    let pid = current.do_clone(frame, flags)?;
    Ok(SyscallResult::Done(pid.as_int() as c_long))
}
