use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::size_t;
use crate::types::sys::socket::MSG_DONTWAIT;
use crate::types::sys::socket::MSG_NOSIGNAL;
use crate::types::sys::socket::SockAddr;

const SUPPORTED_FLAGS: c_int = MSG_DONTWAIT | MSG_NOSIGNAL;

pub fn sys_sendto(
    current: &LxThread,
    fd: c_int,
    buf: *const c_void,
    len: size_t,
    flags: c_int,
    dest_addr: *const u8,
    addr_len: usize,
) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let dest = if dest_addr.is_null() {
        None
    } else {
        Some(SockAddr::parse(dest_addr, addr_len)?)
    };

    let bytes = unsafe { slice::from_raw_parts(buf.cast(), len) };

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let n = file.sendto(bytes, dest, flags)?;
    Ok(n.try_into().unwrap()) // TODO: Better type for sendto return value (usize, but won't exceed c_long)
}
