use ftl::warn;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::time::CLOCK_MONOTONIC;
use crate::types::sys::time::CLOCK_REALTIME;
use crate::types::sys::time::time_t;
use crate::types::sys::time::timespec;

pub fn sys_clock_gettime(
    _current: &LxThread,
    clockid: c_int,
    tp: *mut timespec,
) -> Result<c_long, Errno> {
    if tp.is_null() {
        return Err(Errno::EFAULT);
    }

    match clockid {
        CLOCK_MONOTONIC => {
            let nanos = ftl::time::now().as_nanos();
            unsafe {
                tp.write(timespec {
                    tv_sec: (nanos / 1_000_000_000) as time_t,
                    tv_nsec: (nanos % 1_000_000_000) as c_long,
                });
            }
            Ok(0)
        }
        CLOCK_REALTIME => {
            // TODO: Implement CLOCK_REALTIME properly. Currently the monotonic clock is used.
            let nanos = ftl::time::now().as_nanos();
            unsafe {
                tp.write(timespec {
                    tv_sec: (nanos / 1_000_000_000) as time_t,
                    tv_nsec: (nanos % 1_000_000_000) as c_long,
                });
            }
            Ok(0)
        }
        _ => {
            warn!("clock_gettime: unsupported clockid: {}", clockid);
            Err(Errno::EINVAL)
        }
    }
}
