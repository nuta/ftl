use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::types::sys::capability::CapUserData;
use crate::types::sys::capability::CapUserHeader;
use crate::types::sys::capability::LINUX_CAPABILITY_VERSION_3;

pub fn sys_capget(
    _current: &LxThread,
    header: *mut CapUserHeader,
    data: *mut CapUserData,
) -> Result<SyscallResult, Errno> {
    if header.is_null() {
        return Err(Errno::EFAULT);
    }

    let version = unsafe { (*header).version };

    // Write the version we support.
    unsafe { (*header).version = LINUX_CAPABILITY_VERSION_3 };

    if data.is_null() {
        return Ok(SyscallResult::Done(0));
    }

    if version != LINUX_CAPABILITY_VERSION_3 {
        return Err(Errno::EINVAL);
    }

    // Tell that the process holds every capability.
    let all = CapUserData {
        effective: !0,
        permitted: !0,
        inheritable: 0,
    };

    // Write 2 structs for lower and upper 32-bit capability bits (64 bits total).
    unsafe { data.cast::<[CapUserData; 2]>().write([all; 2]) };
    Ok(SyscallResult::Done(0))
}
