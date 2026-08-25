use crate::thread::Thread;
use crate::types::asm::prctl::ARCH_SET_FS;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::errno::Errno;

pub fn sys_arch_prctl(current: &Thread, code: c_int, addr: c_ulong) -> Result<c_long, Errno> {
    match code {
        ARCH_SET_FS => {
            match current.set_fsbase(addr) {
                Ok(()) => Ok(0),
                Err(_) => Err(Errno::EPERM),
            }
        }
        _ => Err(Errno::EINVAL),
    }
}
