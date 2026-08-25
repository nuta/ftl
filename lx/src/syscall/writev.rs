use core::slice;

use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::uio::iovec;

pub fn sys_writev(
    _current: &Thread,
    fd: c_int,
    iov: *const iovec,
    iovcnt: c_int,
) -> Result<c_long, Errno> {
    // TODO: fd table
    let _ = fd;

    if iovcnt < 0 {
        return Err(Errno::EINVAL);
    }

    let iovecs = unsafe { slice::from_raw_parts(iov, iovcnt as usize) };
    let mut written = 0;
    for iovec in iovecs {
        let ptr = iovec.iov_base.cast::<u8>();
        let bytes = unsafe { slice::from_raw_parts(ptr, iovec.iov_len) };
        ftl::syscall::print(bytes);
        written += iovec.iov_len;
    }

    Ok(written as c_long)
}
