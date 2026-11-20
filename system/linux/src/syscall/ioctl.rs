use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_ulong;
use crate::types::errno::Errno;

pub fn sys_ioctl(
    current: &Thread,
    fd: c_int,
    cmd: c_ulong,
    arg: usize,
) -> Result<SyscallResult, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let retval = file.ioctl(cmd, arg)?;
    Ok(SyscallResult::Done(retval))
}
