use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddr;
use crate::wait_queue::Sleep;

pub fn sys_connect(
    current: &LxThread,
    fd: c_int,
    addr: *const u8,
    addr_len: usize,
) -> Result<SyscallResult, Errno> {
    let addr = SockAddr::parse(addr, addr_len)?;
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.connect(addr, Sleep::Interruptible(&process))?;
    Ok(SyscallResult::Done(0))
}
