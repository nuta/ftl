use core::mem::size_of;

use crate::types::errno::Errno;

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
