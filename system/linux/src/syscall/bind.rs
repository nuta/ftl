use super::SyscallResult;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;

pub fn sys_bind(
    current: &Thread,
    fd: c_int,
    addr: *const u8,
    addr_len: usize,
) -> Result<SyscallResult, Errno> {
    let addr = SockAddr::parse(addr, addr_len)?;
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.bind(addr)?;
    Ok(SyscallResult::Done(0))
}
