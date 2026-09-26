use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_shutdown(current: &LxThread, fd: c_int, how: c_int) -> Result<c_long, Errno> {
    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    file.shutdown(how)?;
    Ok(0)
}
