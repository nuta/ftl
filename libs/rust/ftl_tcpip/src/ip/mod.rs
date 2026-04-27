use core::fmt;

pub mod ipv4;

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
