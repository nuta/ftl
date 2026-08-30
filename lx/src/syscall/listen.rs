use super::socket::listener;
use crate::net::TcpListener;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_listen(current: &Thread, fd: c_int, backlog: c_int) -> Result<c_long, Errno> {
    let file = listener(current, fd)?;
    let listener = file
        .as_any()
        .downcast_ref::<TcpListener>()
        .ok_or(Errno::EINVAL)?;
    listener.listen(backlog)?;
    Ok(0)
}
