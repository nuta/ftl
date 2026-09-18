use crate::process::PId;
use crate::signal::Signal;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_kill(current: &LxThread, pid: c_int, signal: c_int) -> Result<c_long, Errno> {
    let signal = if signal == 0 {
        // If signal is zero, signal won't be delivered but we still need to
        // check if it's deliverable (e.g. the target process exists).
        None
    } else {
        let signal = Signal::from_raw(signal)?;
        Some(signal)
    };

    let process = current.process();
    let target = process
        .container()
        .processes
        .lock()
        .get(PId::new(pid))
        .ok_or(Errno::ESRCH)?;

    if let Some(signal) = signal {
        target.queue_signal(signal)?;
    }

    Ok(0)
}
