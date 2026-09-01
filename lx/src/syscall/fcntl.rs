use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::F_GETFL;
use crate::types::sys::fcntl::F_SETFL;

pub fn sys_fcntl(current: &LxThread, fd: c_int, cmd: c_int, arg: c_long) -> Result<c_long, Errno> {
    let file = {
        let process = current.process();
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    match cmd {
        F_GETFL => Ok(file.flags() as c_long),
        F_SETFL => {
            file.set_status_flags(arg as c_int)?;
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}
