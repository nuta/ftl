use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::time::WallTime;
use ftl::time::WallTimeExt;
use ftl::trace;
use ftl::warn;
use ftl_types::time::Duration;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::time::CLOCK_MONOTONIC;
use crate::types::sys::time::CLOCK_REALTIME;
use crate::types::sys::time::TIMER_ABSTIME;
use crate::types::sys::time::TimeSpec;

const SUPPORTED_FLAGS: c_int = TIMER_ABSTIME;

pub fn sys_clock_nanosleep(
    current: &LxThread,
    clockid: c_int,
    flags: c_int,
    t: *const TimeSpec,
    remain: *mut TimeSpec,
) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    if t.is_null() {
        return Err(Errno::EFAULT);
    }

    let ts = unsafe { t.read() };
    let ts_ns = ts.to_nanos().ok_or(Errno::EINVAL)?;
    let absolute_time = flags & TIMER_ABSTIME != 0;
    let now_mono = MonoTime::now();
    let deadline = match clockid {
        CLOCK_MONOTONIC => {
            let deadline_ns = if absolute_time {
                ts_ns
            } else {
                now_mono.as_nanos().saturating_add(ts_ns)
            };

            MonoTime::from_nanos(deadline_ns)
        }
        CLOCK_REALTIME => {
            let now_wall = WallTime::now();
            let deadline_ns = if absolute_time {
                ts_ns
            } else {
                now_wall.as_nanos().saturating_add(ts_ns)
            };

            let duration_ns = deadline_ns.saturating_sub(now_wall.as_nanos());
            now_mono + Duration::from_nanos(duration_ns)
        }
        _ => {
            warn!("clock_nanosleep: unsupported clockid: {}", clockid);
            return Err(Errno::EINVAL);
        }
    };

    let process = current.process();
    loop {
        if process.has_pending_signal() {
            if !absolute_time && !remain.is_null() {
                // The sleep has been interrupted by a signal. Write the
                // remaining time.
                let now = MonoTime::now();
                let duration = match deadline.duration_since(now) {
                    Some(duration) => duration,
                    None => {
                        trace!("clock_nanosleep: deadline is in the past");
                        Duration::ZERO
                    }
                };

                unsafe {
                    remain.write(TimeSpec::from_duration(duration));
                }
            }

            return Err(Errno::EINTR);
        }

        let guard = process.signal_wait_queue().subscribe();
        if guard.wait_with_deadline(deadline)? {
            return Ok(0);
        }
    }
}
