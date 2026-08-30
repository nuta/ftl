use alloc::sync::Arc;

use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

const AF_INET: c_int = 2;
const SOCK_STREAM: c_int = 1;
const IPPROTO_TCP: c_int = 6;

pub fn sys_socket(
    current: &Thread,
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
) -> Result<c_long, Errno> {
    if domain != AF_INET || socket_type != SOCK_STREAM {
        return Err(Errno::ENOTSUP);
    }
    if protocol != 0 && protocol != IPPROTO_TCP {
        return Err(Errno::ENOTSUP);
    }

    let process = current.process();
    let network = process.container().network();
    let listener = network.create_listener(process.poll());
    let file: Arc<dyn FileLike> = listener;
    Ok(process.fd_table().lock().insert(file)? as c_long)
}

pub(super) fn listener(current: &Thread, fd: c_int) -> Result<Arc<dyn FileLike>, Errno> {
    let process = current.process();
    Ok(process.fd_table().lock().get(fd)?.clone())
}
