use alloc::sync::Arc;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::fcntl::O_RDONLY;
use crate::types::sys::fcntl::O_WRONLY;
use crate::vfs::Pipe;

const SUPPORTED_FLAGS: c_int = O_CLOEXEC | O_NONBLOCK;

pub fn sys_pipe2(current: &LxThread, pipefd: *mut c_int, flags: c_int) -> Result<c_long, Errno> {
    if pipefd.is_null() {
        return Err(Errno::EFAULT);
    }

    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    let (reader, writer) = Pipe::pair()?;
    let process = current.process();
    let (read_fd, write_fd) = {
        let mut fd_table = process.fd_table().lock();
        fd_table.insert2(
            Arc::new(reader),
            flags | O_RDONLY,
            Arc::new(writer),
            flags | O_WRONLY,
        )?
    };

    unsafe {
        *pipefd = read_fd;
        *pipefd.add(1) = write_fd;
    }

    Ok(0)
}
