use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_rt_sigreturn(current: &LxThread, frame: &mut SyscallFrame) -> Result<c_long, Errno> {
    current.return_from_signal(frame)
}
