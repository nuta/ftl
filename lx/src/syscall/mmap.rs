use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_mmap(
    current: &LxThread,
    addr: usize,
    len: usize,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: i64,
) -> Result<c_long, Errno> {
    let addr = current.process().mmap(addr, len, prot, flags, fd, offset)?;
    Ok(addr as c_long)
}
