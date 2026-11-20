use super::SyscallResult;
use crate::signal::SignalSet;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_rt_sigprocmask(
    current: &Thread,
    how: c_int,
    set: *const u64,
    old_set: *mut u64,
    sigset_size: usize,
) -> Result<SyscallResult, Errno> {
    if sigset_size != size_of::<u64>() {
        return Err(Errno::EINVAL);
    }

    let new = if set.is_null() {
        None
    } else {
        let raw = unsafe { set.read() };
        Some(SignalSet::from_raw(raw))
    };

    let old = current.sigprocmask(how, new)?;
    if !old_set.is_null() {
        unsafe { old_set.write(old.to_raw()) };
    }

    Ok(SyscallResult::Done(0))
}
