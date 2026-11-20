use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::stat::Stat;

pub fn sys_fstat(current: &Thread, fd: c_int, buf: *mut Stat) -> Result<SyscallResult, Errno> {
    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let stat = file.stat()?;
    unsafe { buf.write(stat) };
    Ok(SyscallResult::Done(0))
}
