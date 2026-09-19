use alloc::sync::Arc;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_unsigned;
use crate::types::errno::Errno;
use crate::types::sys::eventfd::EFD_CLOEXEC;
use crate::types::sys::eventfd::EFD_NONBLOCK;
use crate::types::sys::eventfd::EFD_SEMAPHORE;
use crate::types::sys::fcntl::O_RDWR;
use crate::vfs::EventFd;

const SUPPORTED_FLAGS: c_int = EFD_CLOEXEC | EFD_NONBLOCK | EFD_SEMAPHORE;
const OPEN_FLAGS: c_int = O_RDWR | EFD_CLOEXEC | EFD_NONBLOCK;

pub fn sys_eventfd2(
    current: &LxThread,
    initval: c_unsigned,
    flags: c_int,
) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    let eventfd = EventFd::new(initval as u64, flags & EFD_SEMAPHORE != 0)?;
    let fd = current
        .process()
        .fd_table()
        .lock()
        .insert(Arc::new(eventfd), O_RDWR | (flags & OPEN_FLAGS))?;
    Ok(fd as c_long)
}
