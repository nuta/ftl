use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::size_t;

pub fn sys_sched_getaffinity(
    _current: &Thread,
    _pid: c_int,
    len: size_t,
    mask: *mut c_ulong,
) -> Result<SyscallResult, Errno> {
    if len < size_of::<c_ulong>() || len % size_of::<c_ulong>() != 0 {
        return Err(Errno::EINVAL);
    }

    if mask.is_null() {
        return Err(Errno::EFAULT);
    }

    // TODO: FTL does not support SMP. Mark only CPU 0 as available.
    unsafe { mask.write(1) };
    let written_len = size_of::<c_ulong>();

    Ok(SyscallResult::Done(written_len as c_long))
}
