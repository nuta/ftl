use core::fmt;

/// An integer in network-endian.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Ne<T>(T);

impl Ne<u16> {
    const fn new(value: u16) -> Self {
        Ne(u16::from_be(value))
    }
}

impl From<Ne<u16>> for u16 {
    fn from(value: Ne<u16>) -> Self {
        u16::from_be(value.0)
    }
}

impl fmt::Debug for Ne<u16> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Into::<u16>::into(*self))
    }
}

impl Ne<u32> {
    const fn new(value: u32) -> Self {
        Ne(u32::from_be(value))
    }
}

impl From<Ne<u32>> for u32 {
    fn from(value: Ne<u32>) -> Self {
        u32::from_be(value.0)
    }
}

impl fmt::Debug for Ne<u32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Into::<u32>::into(*self))
    }
}
