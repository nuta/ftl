use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_shutdown(current: &LxThread, fd: c_int, how: c_int) -> Result<SyscallResult, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.shutdown(how)?;
    Ok(SyscallResult::Done(0))
}
