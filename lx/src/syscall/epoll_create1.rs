use alloc::sync::Arc;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_RDWR;
use crate::vfs::Epoll;

pub fn sys_epoll_create1(current: &LxThread, _flags: c_int) -> Result<c_long, Errno> {
    // TODO: support flags

    let fd = current
        .process()
        .fd_table()
        .lock()
        .insert(Arc::new(Epoll::new()?), O_RDWR)?;
    Ok(fd as c_long)
}
