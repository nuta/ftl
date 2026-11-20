use super::SyscallResult;
use super::clock_nanosleep::sys_clock_nanosleep;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::time::CLOCK_MONOTONIC;
use crate::types::sys::time::TimeSpec;

pub fn sys_nanosleep(
    current: &Thread,
    t: *const TimeSpec,
    remain: *mut TimeSpec,
) -> Result<SyscallResult, Errno> {
    sys_clock_nanosleep(current, CLOCK_MONOTONIC, 0, t, remain)
}
