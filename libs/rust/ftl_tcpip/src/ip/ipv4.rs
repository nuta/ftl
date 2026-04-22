use core::fmt;

use crate::endian::Ne;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const UNSPECIFIED: Self = Self(0);

    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }
}

impl From<Ne<u32>> for Ipv4Addr {
    fn from(raw: Ne<u32>) -> Self {
        let value = raw.into();
        Self(value)
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0 >> 24, (self.0 >> 16) & 0xFF, (self.0 >> 8) & 0xFF, self.0 & 0xFF)
    }
}
