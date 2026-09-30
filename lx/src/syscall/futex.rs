use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::time::WallTime;
use ftl::time::WallTimeExt;
use ftl::trace;
use ftl_types::time::Duration;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::futex::FUTEX_BITSET_MATCH_ANY;
use crate::types::sys::futex::FUTEX_CLOCK_REALTIME;
use crate::types::sys::futex::FUTEX_PRIVATE_FLAG;
use crate::types::sys::futex::FUTEX_WAIT;
use crate::types::sys::futex::FUTEX_WAIT_BITSET;
use crate::types::sys::futex::FUTEX_WAKE;
use crate::types::sys::futex::FUTEX_WAKE_BITSET;
use crate::types::sys::time::TimeSpec;
use crate::wait_queue::Sleep;

const FUTEX_OP_FLAGS: c_int = FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME;

pub fn sys_futex(
    current: &LxThread,
    uaddr: *mut u32,
    op: c_int,
    val: u32,
    timeout: *const TimeSpec,
    val3: u32,
) -> Result<SyscallResult, Errno> {
    if uaddr.is_null() {
        return Err(Errno::EFAULT);
    }

    // The address must be aligned to 4 bytes.
    if !uaddr.is_aligned() {
        return Err(Errno::EINVAL);
    }

    let cmd = op & !FUTEX_OP_FLAGS;
    let bitset = if cmd == FUTEX_WAIT_BITSET || cmd == FUTEX_WAKE_BITSET {
        val3
    } else {
        FUTEX_BITSET_MATCH_ANY
    };

    if bitset == 0 {
        // The bitset must be non-zero.
        return Err(Errno::EINVAL);
    }

    let process = current.process();
    let futexes = process.futexes();
    match cmd {
        FUTEX_WAIT | FUTEX_WAIT_BITSET => {
            let deadline = if timeout.is_null() {
                None
            } else {
                let ts = unsafe { timeout.read() };
                let ns = ts.to_nanos().ok_or(Errno::EINVAL)?;

                let realtime = op & FUTEX_CLOCK_REALTIME != 0;
                let absolute = cmd == FUTEX_WAIT_BITSET;
                let now = MonoTime::now();
                let deadline = if absolute {
                    if realtime {
                        // Realtime clock.
                        let duration_ns = ns.saturating_sub(WallTime::now().as_nanos());
                        now + Duration::from_nanos(duration_ns)
                    } else {
                        // Monotonic clock.
                        MonoTime::from_nanos(ns)
                    }
                } else {
                    // Relative time.
                    now + Duration::from_nanos(ns)
                };

                Some(deadline)
            };

            let sleep = Sleep::Interruptible(&process);
            futexes.wait(uaddr, val, deadline, bitset, sleep)?;
            Ok(SyscallResult::Done(0))
        }
        FUTEX_WAKE | FUTEX_WAKE_BITSET => {
            let count = futexes.wake(uaddr.addr(), val as usize, bitset)?;
            Ok(SyscallResult::Done(count as c_long))
        }
        _ => {
            trace!("futex: unsupported op {:#x}", op);
            Err(Errno::ENOSYS)
        }
    }
}
