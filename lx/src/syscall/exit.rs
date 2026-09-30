use ftl_types::thread::ExitReason;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_exit(current: &LxThread, status: c_int) -> Result<SyscallResult, Errno> {
    let reason = match status {
        0 => ExitReason::Success,
        _ => ExitReason::Errored,
    };

    current.set_exit_status(status);
    Ok(SyscallResult::Exit(reason))
}
