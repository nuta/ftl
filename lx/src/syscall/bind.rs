use super::socket::listener;
use crate::net::TcpListener;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;

const AF_INET: u16 = 2;
const SOCKADDR_IN_LEN: usize = 16;

pub fn sys_bind(
    current: &Thread,
    fd: c_int,
    address: *const u8,
    address_len: usize,
) -> Result<c_long, Errno> {
    if address.is_null() || address_len < SOCKADDR_IN_LEN {
        return Err(Errno::EINVAL);
    }

    let address = unsafe { core::slice::from_raw_parts(address, address_len) };
    let family = u16::from_ne_bytes(address[0..2].try_into().unwrap());
    if family != AF_INET {
        return Err(Errno::EINVAL);
    }

    let port = u16::from_be_bytes(address[2..4].try_into().unwrap());
    let file = listener(current, fd)?;
    let listener = file
        .as_any()
        .downcast_ref::<TcpListener>()
        .ok_or(Errno::EINVAL)?;
    listener.bind(port)?;
    Ok(0)
}
