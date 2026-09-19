use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_setsockopt(
    current: &LxThread,
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *const u8,
    optlen: usize,
) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.setsockopt(level, optname, optval, optlen)?;
    Ok(0)
}
