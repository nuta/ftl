use super::SyscallResult;
use super::fchownat::convert_id;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

pub fn sys_fchown(current: &Thread, fd: c_int, uid: u32, gid: u32) -> Result<SyscallResult, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.file().chown(convert_id(uid), convert_id(gid))?;
    Ok(SyscallResult::Done(0))
}
