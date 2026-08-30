use alloc::sync::Arc;

use super::socket::listener;
use crate::net::TcpListener;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

pub fn sys_accept(
    current: &Thread,
    fd: c_int,
    _address: *mut u8,
    _address_len: *mut u32,
) -> Result<c_long, Errno> {
    let file = listener(current, fd)?;
    let listener = file
        .as_any()
        .downcast_ref::<TcpListener>()
        .ok_or(Errno::EINVAL)?;
    let connection = listener.accept()?;
    let connection: Arc<dyn FileLike> = connection;
    let process = current.process();
    Ok(process.fd_table().lock().insert(connection)? as c_long)
}
