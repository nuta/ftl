use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::resource::RLIM_INFINITY;
use crate::types::sys::resource::RLIMIT_NOFILE;
use crate::types::sys::resource::RLIMIT_STACK;
use crate::types::sys::resource::RLimit;
use crate::vm::STACK_SIZE;

const RLIM_NLIMITS: u32 = 16;

pub fn sys_prlimit64(
    current: &Thread,
    pid: c_int,
    resource: u32,
    new_limit: *const RLimit,
    old_limit: *mut RLimit,
) -> Result<SyscallResult, Errno> {
    if pid != 0 && pid != current.process().id().as_int() {
        return Err(Errno::EPERM);
    }

    if resource >= RLIM_NLIMITS {
        return Err(Errno::EINVAL);
    }

    if !new_limit.is_null() {
        return Err(Errno::EPERM);
    }

    if !old_limit.is_null() {
        let limit = match resource {
            RLIMIT_STACK => STACK_SIZE as u64,
            RLIMIT_NOFILE => current.process().fd_table().lock().capacity() as u64,
            _ => RLIM_INFINITY,
        };

        unsafe {
            old_limit.write(RLimit {
                rlim_cur: limit,
                rlim_max: limit,
            })
        };
    }

    Ok(SyscallResult::Done(0))
}
