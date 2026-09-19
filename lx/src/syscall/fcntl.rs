use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::F_DUPFD;
use crate::types::sys::fcntl::F_DUPFD_CLOEXEC;
use crate::types::sys::fcntl::F_GETFD;
use crate::types::sys::fcntl::F_GETFL;
use crate::types::sys::fcntl::F_SETFD;
use crate::types::sys::fcntl::F_SETFL;
use crate::types::sys::fcntl::FD_CLOEXEC;

pub fn sys_fcntl(current: &LxThread, fd: c_int, cmd: c_int, arg: c_long) -> Result<c_long, Errno> {
    let process = current.process();
    let mut fd_table = process.fd_table().lock();

    match cmd {
        F_DUPFD => Ok(fd_table.dup(fd, arg as c_int, false)? as c_long),
        F_DUPFD_CLOEXEC => Ok(fd_table.dup(fd, arg as c_int, true)? as c_long),
        F_GETFD => {
            let mut retval = 0;
            if fd_table.get_cloexec(fd)? {
                retval |= FD_CLOEXEC;
            }

            Ok(retval as c_long)
        }
        F_SETFD => {
            fd_table.set_cloexec(fd, arg as c_int & FD_CLOEXEC != 0)?;
            Ok(0)
        }
        F_GETFL => {
            let file = fd_table.get(fd)?;
            Ok(file.flags() as c_long)
        }
        F_SETFL => {
            let file = fd_table.get(fd)?;
            file.set_status_flags(arg as c_int)?;
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}
