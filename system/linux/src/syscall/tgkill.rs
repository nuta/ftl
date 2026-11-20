use super::SyscallResult;
use crate::process::PId;
use crate::signal::Signal;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_tgkill(
    current: &Thread,
    tgid: c_int,
    tid: c_int,
    signal: c_int,
) -> Result<SyscallResult, Errno> {
    if tgid <= 0 || tid <= 0 {
        return Err(Errno::EINVAL);
    }

    let signal = if signal == 0 {
        None
    } else {
        Some(Signal::from_raw(signal)?)
    };

    // Find the target process.
    let process = current
        .process()
        .global()
        .processes
        .lock()
        .get(PId::new(tgid))
        .ok_or(Errno::ESRCH)?;

    if process.id() != PId::new(tgid) {
        return Err(Errno::ESRCH);
    }

    // Find the target thread.
    let thread = process
        .get_thread_by_tid(PId::new(tid))
        .ok_or(Errno::ESRCH)?;

    if let Some(signal) = signal {
        thread.queue_signal(signal)?;
    }

    Ok(SyscallResult::Done(0))
}
