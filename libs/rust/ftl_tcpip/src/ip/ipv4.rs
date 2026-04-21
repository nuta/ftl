#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const UNSPECIFIED: Self = Self(0);

    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }
}
