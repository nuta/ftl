use alloc::sync::Arc;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::wait_queue::Sleep;

pub fn sys_close(current: &LxThread, fd: c_int) -> Result<SyscallResult, Errno> {
    let process = current.process();

    // Note: Do not call `file.close()` here. Other forked processes may still
    // have a reference to this file. Arc<OpenFile> will handle the clean-up.
    let file = process.fd_table().lock().remove(fd)?;

    // Is this the only reference to the file?
    if let Some(file) = Arc::into_inner(file) {
        // If the linger is set, wait for it to complete.
        file.linger(Sleep::Interruptible(&process))?;
    }

    Ok(SyscallResult::Done(0))
}
