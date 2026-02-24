use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Errno(i32);

impl Errno {
    pub const ENOSYS: Self = Self(38);
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Errno::ENOSYS => write!(f, "Function not implemented"),
            _ => write!(f, "Unknown error"),
        }
    }
}
