use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::asm::prctl::ARCH_SET_FS;
use crate::types::c_int;
use crate::types::c_ulong;
use crate::types::errno::Errno;

pub fn sys_arch_prctl(
    current: &LxThread,
    code: c_int,
    addr: c_ulong,
) -> Result<SyscallResult, Errno> {
    match code {
        ARCH_SET_FS => {
            match current.set_fsbase(addr) {
                Ok(()) => Ok(SyscallResult::Done(0)),
                Err(_) => Err(Errno::EPERM),
            }
        }
        _ => Err(Errno::EINVAL),
    }
}
