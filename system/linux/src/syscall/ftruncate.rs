use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::sys::fcntl::O_ACCMODE;
use crate::types::sys::fcntl::O_RDONLY;

pub fn sys_ftruncate(current: &Thread, fd: c_int, len: off_t) -> Result<SyscallResult, Errno> {
    let Ok(len) = usize::try_from(len) else {
        return Err(Errno::EINVAL);
    };

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let access_mode = file.flags() & O_ACCMODE;
    if access_mode == O_RDONLY {
        return Err(Errno::EINVAL);
    }

    file.file().truncate(len)?;
    Ok(SyscallResult::Done(0))
}
