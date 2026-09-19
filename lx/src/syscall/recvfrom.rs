use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;
use crate::types::sys::socket::MSG_DONTWAIT;
use crate::types::sys::socket::MSG_NOSIGNAL;
use crate::types::sys::socket::write_sockaddr;

const SUPPORTED_FLAGS: c_int = MSG_DONTWAIT | MSG_NOSIGNAL;

pub fn sys_recvfrom(
    current: &LxThread,
    fd: c_int,
    buf: *mut c_void,
    len: size_t,
    flags: c_int,
    src_addr: *mut u8,
    addr_len: *mut u32,
) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let bytes = unsafe { slice::from_raw_parts_mut(buf.cast(), len) };

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let (n, addr) = file.recvfrom(bytes, flags)?;
    if !src_addr.is_null() {
        write_sockaddr(src_addr, addr_len, &addr.as_raw())?;
    }

    Ok(n as c_long)
}
