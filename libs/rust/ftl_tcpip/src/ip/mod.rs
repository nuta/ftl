pub mod ipv4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddr {
    V4(ipv4::Ipv4Addr),
}
