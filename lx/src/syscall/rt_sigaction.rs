use crate::signal::SigAction;
use crate::signal::Signal;
use crate::thread::LxThread;
use crate::types;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_rt_sigaction(
    current: &LxThread,
    signal: c_int,
    action: *const types::signal::SigAction,
    old_action: *mut types::signal::SigAction,
    sigset_size: usize,
) -> Result<c_long, Errno> {
    if sigset_size != size_of::<u64>() {
        return Err(Errno::EINVAL);
    }

    let signal = Signal::from_raw(signal)?;
    if signal.is_uncatchable() {
        return Err(Errno::EINVAL);
    }

    let new_action = if action.is_null() {
        None
    } else {
        let raw = unsafe { action.read() };
        Some(SigAction::from_raw(raw))
    };

    let old = current.process().sigaction(signal, new_action)?;
    if !old_action.is_null() {
        unsafe { old_action.write(old.to_raw()) };
    }

    Ok(0)
}
