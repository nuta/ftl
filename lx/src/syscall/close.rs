use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_close(current: &Thread, fd: c_int) -> Result<c_long, Errno> {
    let process = current.process();
    // Note: Do not call `file.close()` here. Other forked processes may still
    // have a reference to this file. Arc<OpenFile> will handle the clean-up.
    let _file = process.fd_table().lock().remove(fd)?;
    Ok(0)
}
