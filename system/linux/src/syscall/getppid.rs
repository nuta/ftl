use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_getppid(current: &Thread) -> Result<SyscallResult, Errno> {
    let ppid = match current.process().parent_id() {
        Some(id) => id.as_int(),
        None => 0, // The parent of the init process.
    };

    Ok(SyscallResult::Done(ppid as c_long))
}
