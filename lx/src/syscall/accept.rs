use core::cmp::min;
use core::mem::size_of;
use core::slice;

use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::socket::SockAddrIn;

pub fn sys_accept(
    current: &Thread,
    fd: c_int,
    addr: *mut u8,
    addr_len: *mut u32,
) -> Result<c_long, Errno> {
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

    // Add the accepted socket to fhe file descriptor table.
    let conn_fd = match process.fd_table().lock().insert(conn.clone()) {
        Ok(fd) => fd,
        Err(error) => {
            conn.close();
            return Err(error);
        }
    };

    Ok(conn_fd as c_long)
}

fn write_sockaddr(
    addr: *mut u8,
    addr_len: *mut u32,
    sockaddr_in: &SockAddrIn,
) -> Result<(), Errno> {
    if addr_len.is_null() {
        return Err(Errno::EINVAL);
    }

    // Truncate the copy length to the user-provided buffer size.
    let sockaddr_len = size_of::<SockAddrIn>();
    let buf_len = unsafe { addr_len.read_unaligned() } as usize;
    let copy_len = min(buf_len, sockaddr_len);

    // Copy the socket address to the buffer.
    unsafe {
        let src = slice::from_raw_parts(
            sockaddr_in as *const SockAddrIn as *const u8,
            sockaddr_len,
        );
        addr.copy_from_nonoverlapping(src.as_ptr(), copy_len);
        addr_len.write_unaligned(sockaddr_len as u32);
    }

    Ok(())
}
