use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::time::CLOCK_BOOTTIME;
use crate::types::sys::time::CLOCK_MONOTONIC;
use crate::types::sys::time::CLOCK_MONOTONIC_COARSE;
use crate::types::sys::time::CLOCK_MONOTONIC_RAW;
use crate::types::sys::time::CLOCK_REALTIME;
use crate::types::sys::time::CLOCK_REALTIME_COARSE;
use crate::types::sys::time::TimeSpec;

pub fn sys_clock_getres(
    _current: &LxThread,
    clockid: c_int,
    res: *mut TimeSpec,
) -> Result<SyscallResult, Errno> {
    match clockid {
        CLOCK_REALTIME
        | CLOCK_MONOTONIC
        | CLOCK_MONOTONIC_RAW
        | CLOCK_REALTIME_COARSE
        | CLOCK_MONOTONIC_COARSE
        | CLOCK_BOOTTIME => {}
        _ => return Err(Errno::EINVAL),
    }

    if !res.is_null() {
        // 1ns resolution.
        // TODO: Should we return an accurate resolution of timer?
        unsafe { res.write(TimeSpec::from_nanos(1)) };
    }

    Ok(SyscallResult::Done(0))
}
