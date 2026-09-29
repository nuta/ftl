use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_mprotect(
    current: &LxThread,
    addr: usize,
    len: usize,
    prot: c_int,
) -> Result<SyscallResult, Errno> {
    current.vm().mprotect(addr, len, prot)?;
    Ok(SyscallResult::Done(0))
}
