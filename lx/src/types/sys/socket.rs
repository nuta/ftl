use core::cmp::min;
use core::mem::size_of;
use core::slice;

use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_NONBLOCK;

pub const SOCK_NONBLOCK: c_int = O_NONBLOCK;
pub const SOCK_CLOEXEC: c_int = O_CLOEXEC;

pub const SOL_SOCKET: c_int = 1;
pub const SO_REUSEADDR: c_int = 2;

pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_NOSIGNAL: c_int = 0x4000;

const AF_INET: u16 = 2;

#[derive(Clone, Copy)]
#[repr(C)]
struct InAddr {
    s_addr: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: InAddr,
    sin_zero: [u8; 8],
}

pub enum SockAddr {
    Inet { ip: u32, port: u16 },
}

impl SockAddr {
    pub fn parse(addr: *const u8, addr_len: usize) -> Result<Self, Errno> {
        if addr.is_null() || addr_len < size_of::<SockAddrIn>() {
            return Err(Errno::EINVAL);
        }

        let addr = unsafe { addr.cast::<SockAddrIn>().read_unaligned() };
        if addr.sin_family != AF_INET {
            return Err(Errno::EINVAL);
        }

        Ok(Self::Inet {
            ip: u32::from_be(addr.sin_addr.s_addr),
            port: u16::from_be(addr.sin_port),
        })
    }

    pub fn as_raw(&self) -> SockAddrIn {
        let Self::Inet { ip, port } = self;
        SockAddrIn {
            sin_family: AF_INET,
            sin_port: port.to_be(),
            sin_addr: InAddr { s_addr: ip.to_be() },
            sin_zero: [0; 8],
        }
    }
}

pub fn write_sockaddr(
    addr: *mut u8,
    addr_len: *mut u32,
    sockaddr_in: &SockAddrIn,
) -> Result<(), Errno> {
    if addr_len.is_null() {
        return Err(Errno::EFAULT);
    }

    // Truncate the copy length to the user-provided buffer size.
    let sockaddr_len = size_of::<SockAddrIn>();
    let buf_len = unsafe { addr_len.read_unaligned() } as usize;
    let copy_len = min(buf_len, sockaddr_len);

    // Copy the socket address to the buffer.
    unsafe {
        let src =
            slice::from_raw_parts(sockaddr_in as *const SockAddrIn as *const u8, sockaddr_len);
        addr.copy_from_nonoverlapping(src.as_ptr(), copy_len);
        addr_len.write_unaligned(sockaddr_len as u32);
    }

    Ok(())
}
