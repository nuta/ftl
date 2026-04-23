use core::fmt;

use crate::endian::Ne;

pub mod tcp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Port(u16);

impl Port {
    pub const fn new(port: u16) -> Self {
        Self(port)
    }
}

impl fmt::Debug for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Ne<u16>> for Port {
    fn from(value: Ne<u16>) -> Self {
        Self(value.into())
    }
}

impl From<Port> for Ne<u16> {
    fn from(value: Port) -> Self {
        value.0.into()
    }
}
