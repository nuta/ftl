use alloc::sync::Arc;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EPOLL_CLOEXEC;
use crate::types::sys::fcntl::O_RDWR;
use crate::vfs::Epoll;

const SUPPORTED_FLAGS: c_int = EPOLL_CLOEXEC;

pub fn sys_epoll_create1(current: &LxThread, flags: c_int) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    let fd = current
        .process()
        .fd_table()
        .lock()
        .insert(Arc::new(Epoll::new()?), O_RDWR | flags)?;
    Ok(fd as c_long)
}
