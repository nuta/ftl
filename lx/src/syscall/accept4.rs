use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::fcntl::O_RDWR;
use crate::types::sys::socket::SOCK_CLOEXEC;
use crate::types::sys::socket::SOCK_NONBLOCK;
use crate::types::sys::socket::write_sockaddr;

const SUPPORTED_FLAGS: c_int = SOCK_CLOEXEC | SOCK_NONBLOCK;

pub fn sys_accept4(
    current: &LxThread,
    fd: c_int,
    addr: *mut u8,
    addr_len: *mut u32,
    flags: c_int,
) -> Result<c_long, Errno> {
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }

    let process = current.process();
    let file = {
        let fd_table = process.fd_table().lock();
        fd_table.get(fd)?.clone()
    };

    // Wait for a new connection...
    let conn = file.accept()?;

    // Write the socket address if a buffer is provided.
    if !addr.is_null() {
        let sockaddr_in = match conn.peer_addr() {
            Ok(sockaddr) => sockaddr.as_raw(),
            Err(e) => {
                conn.close();
                return Err(e);
            }
        };

        if let Err(e) = write_sockaddr(addr, addr_len, &sockaddr_in) {
            conn.close();
            return Err(e);
        }
    }

    let mut fd_flags = O_RDWR;
    if flags & SOCK_NONBLOCK != 0 {
        fd_flags |= O_NONBLOCK;
    }

    if flags & SOCK_CLOEXEC != 0 {
        fd_flags |= O_CLOEXEC;
    }

    // Add the accepted socket to the file descriptor table.
    let conn_fd = match process.fd_table().lock().insert(conn.clone(), fd_flags) {
        Ok(fd) => fd,
        Err(error) => {
            conn.close();
            return Err(error);
        }
    };

    Ok(conn_fd as c_long)
}
