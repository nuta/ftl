use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

const AF_INET: c_int = 2;
const SOCK_STREAM: c_int = 1;
const IPPROTO_TCP: c_int = 6;

pub fn sys_socket(
    current: &Thread,
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
) -> Result<c_long, Errno> {
    if domain != AF_INET {
        return Err(Errno::ENOTSUP);
    }

    if socket_type != SOCK_STREAM {
        return Err(Errno::ENOTSUP);
    }

    if protocol != 0 && protocol != IPPROTO_TCP {
        return Err(Errno::ENOTSUP);
    }

    let process = current.process();
    let network = process.container().network();

    // FIXME: support other socket types
    let listener = network.create_listener().map_err(Errno::from)?;

    let fd = process.fd_table().lock().insert(listener)?;
    Ok(fd as c_long)
}
