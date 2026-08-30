use crate::net::TcpConnection;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

pub fn sys_close(current: &Thread, fd: c_int) -> Result<c_long, Errno> {
    let process = current.process();
    let file = process.fd_table().lock().remove(fd)?;
    // TODO: Support other file types
    if let Some(connection) = file.as_any().downcast_ref::<TcpConnection>() {
        connection.close();
    }
    Ok(0)
}
