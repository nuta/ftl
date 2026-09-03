use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::uio::iovec;

pub fn sys_writev(
    current: &LxThread,
    fd: c_int,
    iov: *const iovec,
    iovcnt: c_int,
) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    if iovcnt == 0 {
        return Ok(0);
    }

    if iovcnt < 0 {
        return Err(Errno::EINVAL);
    }

    let iovecs = unsafe { slice::from_raw_parts(iov, iovcnt as usize) };
    let mut total = 0;
    for iovec in iovecs {
        if iovec.iov_len == 0 {
            continue;
        }

        let ptr = iovec.iov_base.cast::<u8>();
        let bytes = unsafe { slice::from_raw_parts(ptr, iovec.iov_len) };

        let n = file.write(bytes, 0)?;
        total += n;
        if n < iovec.iov_len {
            break;
        }
    }

    Ok(total.try_into().unwrap()) // FIXME: Handle overflow
}
