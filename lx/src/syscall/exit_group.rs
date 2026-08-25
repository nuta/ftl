use ftl_types::thread::ExitReason;

use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_exit_group(_current: &Thread, status: c_int) -> Result<c_long, Errno> {
    let reason = match status {
        0 => ExitReason::Success,
        _ => ExitReason::Errored,
    };

    // TODO: temrinate other threads too
    ftl::syscall::thread_exit(reason)
}
