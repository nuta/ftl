use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::off_t;

pub fn sys_lseek(
    current: &LxThread,
    fd: c_int,
    offset: off_t,
    whence: c_int,
) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    let new_offset = file.seek(offset, whence)?;
    Ok(new_offset as c_long)
}
