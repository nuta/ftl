use core::fmt;

pub(crate) mod ipv4;

pub use ipv4::Ipv4Addr;
pub use ipv4::NetMask;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddr {
    V4(ipv4::Ipv4Addr),
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddr::V4(addr) => write!(f, "{}", addr),
        }
    }
}
