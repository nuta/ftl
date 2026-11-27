use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_fsync(current: &Thread, fd: c_int) -> Result<SyscallResult, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.file().fsync()?;
    Ok(SyscallResult::Done(0))
}
