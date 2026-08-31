use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;

pub fn sys_bind(
    current: &LxThread,
    fd: c_int,
    addr: *const u8,
    addr_len: usize,
) -> Result<c_long, Errno> {
    let addr = SockAddr::parse(addr, addr_len)?;
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.bind(addr)?;
    Ok(0)
}
