use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::socket::write_sockaddr;

pub fn sys_getsockname(
    current: &LxThread,
    fd: c_int,
    addr: *mut u8,
    addr_len: *mut u32,
) -> Result<SyscallResult, Errno> {
    if addr.is_null() {
        return Err(Errno::EFAULT);
    }

    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let sockaddr = file.local_addr()?;
    write_sockaddr(addr, addr_len, &sockaddr.as_raw())?;
    Ok(SyscallResult::Done(0))
}
