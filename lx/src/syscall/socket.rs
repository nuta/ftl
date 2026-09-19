use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::fcntl::O_RDWR;

const AF_INET: c_int = 2;
const SOCK_STREAM: c_int = 1;
const SOCK_CLOEXEC: c_int = O_CLOEXEC;
const SOCK_NONBLOCK: c_int = O_NONBLOCK;
const IPPROTO_TCP: c_int = 6;

const SUPPORTED_FLAGS: c_int = SOCK_CLOEXEC | SOCK_NONBLOCK;

pub fn sys_socket(
    current: &LxThread,
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
) -> Result<c_long, Errno> {
    if domain != AF_INET {
        return Err(Errno::ENOTSUP);
    }

    if socket_type & !SUPPORTED_FLAGS != SOCK_STREAM {
        return Err(Errno::ENOTSUP);
    }

    if protocol != 0 && protocol != IPPROTO_TCP {
        return Err(Errno::ENOTSUP);
    }

    let process = current.process();
    let network = process.container().network();

    // FIXME: support other socket types
    let listener = network.create_listener().map_err(Errno::from)?;

    let fd = process
        .fd_table()
        .lock()
        .insert(listener, O_RDWR | (socket_type & SUPPORTED_FLAGS))?;
    Ok(fd as c_long)
}
