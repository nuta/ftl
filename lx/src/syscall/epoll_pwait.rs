use super::epoll_wait::sys_epoll_wait;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EpollEvent;

pub fn sys_epoll_pwait(
    current: &LxThread,
    epfd: c_int,
    events: *mut EpollEvent,
    max_events: c_int,
    timeout: c_int,
    _sigmask: *const c_void,
) -> Result<c_long, Errno> {
    // TODO: Implement signmask
    sys_epoll_wait(current, epfd, events, max_events, timeout)
}
