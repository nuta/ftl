use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::uio::IoVec;
use crate::vfs::IoVecSlice;
use crate::wait_queue::Sleep;

pub fn sys_readv(
    current: &Thread,
    fd: c_int,
    iov: *const IoVec,
    iovcnt: c_int,
) -> Result<SyscallResult, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if iovcnt == 0 {
        return Ok(SyscallResult::Done(0));
    }

    if iovcnt < 0 {
        return Err(Errno::EINVAL);
    }

    if iov.is_null() {
        return Err(Errno::EFAULT);
    }

    let iovecs = IoVecSlice::new(iov, iovcnt as usize);
    let n = file.readv(
        &iovecs,
        Sleep::Interruptible {
            thread: current,
            process: &process,
        },
    )?;
    Ok(SyscallResult::Done(n as c_long))
}
