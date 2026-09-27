use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::errno::Errno;

pub fn sys_rt_sigreturn(
    current: &LxThread,
    frame: &mut SyscallFrame,
) -> Result<SyscallResult, Errno> {
    let retval = current.return_from_signal(frame)?;
    Ok(SyscallResult::Done(retval))
}
