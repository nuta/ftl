use super::SyscallResult;
use crate::arch::SyscallFrame;
use crate::arch::UserContext;
use crate::thread::Thread;
use crate::types::errno::Errno;

pub fn sys_rt_sigreturn(current: &Thread, frame: &SyscallFrame) -> Result<SyscallResult, Errno> {
    let (ctx, sigmask) = unsafe { UserContext::return_from_signal(frame.sp()) };
    current.set_signal_mask(sigmask);
    Ok(SyscallResult::Restore(ctx))
}
