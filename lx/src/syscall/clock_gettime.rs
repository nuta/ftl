use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::time::WallTime;
use ftl::time::WallTimeExt;
use ftl::warn;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::time::CLOCK_MONOTONIC;
use crate::types::sys::time::CLOCK_REALTIME;
use crate::types::sys::time::TimeSpec;

pub fn sys_clock_gettime(
    _current: &LxThread,
    clockid: c_int,
    tp: *mut TimeSpec,
) -> Result<c_long, Errno> {
    if tp.is_null() {
        return Err(Errno::EFAULT);
    }

    let nanos = match clockid {
        CLOCK_MONOTONIC => MonoTime::now().as_nanos(),
        CLOCK_REALTIME => WallTime::now().as_nanos(),
        _ => {
            warn!("clock_gettime: unsupported clockid: {}", clockid);
            return Err(Errno::EINVAL);
        }
    };

    unsafe {
        tp.write(TimeSpec::from_nanos(nanos));
    }
    Ok(0)
}
