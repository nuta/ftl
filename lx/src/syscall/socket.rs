use alloc::sync::Arc;

use super::SyscallResult;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::fcntl::O_RDWR;
use crate::vfs::FileLike;

const AF_INET: c_int = 2;
const SOCK_STREAM: c_int = 1;
const SOCK_DGRAM: c_int = 2;
const SOCK_CLOEXEC: c_int = O_CLOEXEC;
const SOCK_NONBLOCK: c_int = O_NONBLOCK;
const IPPROTO_TCP: c_int = 6;
const IPPROTO_UDP: c_int = 17;

const SUPPORTED_FLAGS: c_int = SOCK_CLOEXEC | SOCK_NONBLOCK;

pub fn sys_socket(
    current: &LxThread,
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
) -> Result<SyscallResult, Errno> {
    if domain != AF_INET {
        return Err(Errno::ENOTSUP);
    }

    let process = current.process();
    let network = process.container().network();

    // FIXME: support other socket types
    let socket: Arc<dyn FileLike> = match (socket_type & !SUPPORTED_FLAGS, protocol) {
        (SOCK_STREAM, 0 | IPPROTO_TCP) => network.create_tcp_socket().map_err(Errno::from)?,
        (SOCK_DGRAM, 0 | IPPROTO_UDP) => network.create_udp_socket().map_err(Errno::from)?,
        _ => return Err(Errno::ENOTSUP),
    };

    let fd = process
        .fd_table()
        .lock()
        .insert(socket, O_RDWR | (socket_type & SUPPORTED_FLAGS))?;
    Ok(SyscallResult::Done(fd as c_long))
}
