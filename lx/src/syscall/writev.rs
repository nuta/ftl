use alloc::vec::Vec;
use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::uio::IoVec;
use crate::vfs::IoVecSlice;

pub fn sys_writev(
    current: &LxThread,
    fd: c_int,
    iov: *const IoVec,
    iovcnt: c_int,
) -> Result<c_long, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if iovcnt == 0 {
        return Ok(0);
    }

    if iovcnt < 0 {
        return Err(Errno::EINVAL);
    }

    let iovecs = IoVecSlice::new(iov, iovcnt as usize);
    let n = file.writev(&process, &iovecs)?;
    Ok(n.try_into().unwrap()) // FIXME: Handle overflow
}
