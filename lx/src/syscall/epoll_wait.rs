use core::slice;

use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EpollEvent;
use crate::wait_queue::Sleep;

pub fn sys_epoll_wait(
    current: &LxThread,
    epfd: c_int,
    events: *mut EpollEvent,
    max_events: c_int,
    timeout: c_int,
) -> Result<c_long, Errno> {
    if events.is_null() {
        return Err(Errno::EFAULT);
    }

    if max_events <= 0 {
        return Err(Errno::EINVAL);
    }

    let process = current.process();
    let epfile = {
        let fd_table = process.fd_table().lock();
        fd_table.get(epfd)?.clone()
    };

    let events = unsafe { slice::from_raw_parts_mut(events, max_events as usize) };
    let epoll = epfile.as_epoll().ok_or(Errno::EINVAL)?;
    let n = epoll.wait(events, timeout, Sleep::Interruptible(&process))?;
    Ok(n as c_long)
}
