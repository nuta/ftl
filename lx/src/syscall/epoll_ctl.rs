use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EPOLL_CTL_ADD;
use crate::types::sys::epoll::EPOLL_CTL_DEL;
use crate::types::sys::epoll::EPOLL_CTL_MOD;
use crate::types::sys::epoll::EpollEvent;

fn read_event(event: *const EpollEvent) -> Result<EpollEvent, Errno> {
    if event.is_null() {
        return Err(Errno::EFAULT);
    }

    Ok(unsafe { *event })
}

pub fn sys_epoll_ctl(
    current: &LxThread,
    epfd: c_int,
    op: c_int,
    fd: c_int,
    event: *const EpollEvent,
) -> Result<c_long, Errno> {
    let process = current.process();
    let (epfile, file) = {
        let fd_table = process.fd_table().lock();
        (fd_table.get(epfd)?.clone(), fd_table.get(fd)?.clone())
    };

    let epoll = epfile.as_epoll().ok_or(Errno::EINVAL)?;
    match op {
        EPOLL_CTL_ADD => {
            let event = read_event(event)?;
            epoll.add(fd, event, file)?
        }
        EPOLL_CTL_MOD => {
            let event = read_event(event)?;
            epoll.modify(fd, event)?
        }
        EPOLL_CTL_DEL => epoll.delete(fd)?,
        _ => return Err(Errno::EINVAL),
    }

    Ok(0)
}
