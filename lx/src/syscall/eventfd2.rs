use alloc::sync::Arc;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_unsigned;
use crate::types::errno::Errno;
use crate::types::sys::eventfd::EFD_NONBLOCK;
use crate::types::sys::eventfd::EFD_SEMAPHORE;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::fcntl::O_RDWR;
use crate::vfs::EventFd;

pub fn sys_eventfd2(
    current: &LxThread,
    initval: c_unsigned,
    flags: c_int,
) -> Result<c_long, Errno> {
    // TODO: Support other flags
    let mut fd_flags = O_RDWR;
    if flags & EFD_NONBLOCK != 0 {
        fd_flags |= O_NONBLOCK;
    }

    let eventfd = EventFd::new(initval as u64, flags & EFD_SEMAPHORE != 0)?;
    let fd = current
        .process()
        .fd_table()
        .lock()
        .insert(Arc::new(eventfd), fd_flags)?;
    Ok(fd as c_long)
}
