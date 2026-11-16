use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::types::signal::StackT;

pub fn sys_sigaltstack(
    current: &LxThread,
    frame: &SyscallFrame,
    ss: *const StackT,
    old_ss: *mut StackT,
) -> Result<SyscallResult, Errno> {
    let new = if ss.is_null() {
        None
    } else {
        Some(unsafe { ss.read() })
    };

    let old = current.sigaltstack(new, frame.sp())?;
    if !old_ss.is_null() {
        unsafe { old_ss.write(old) };
    }

    Ok(SyscallResult::Done(0))
}
