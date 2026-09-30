use ftl_types::thread::ExitReason;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_exit_group(current: &LxThread, status: c_int) -> Result<SyscallResult, Errno> {
    let reason = match status {
        0 => ExitReason::Success,
        _ => ExitReason::Errored,
    };

    current.process().exit(status)?;

    // FIXME: Terminate other threads too
    Ok(SyscallResult::Exit(reason))
}
