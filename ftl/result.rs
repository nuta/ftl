#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    AlreadyExists = -1,
    ClosedByPeer = -2,
    Empty = -3,
    Full = -4,
}

pub type Result<T> = ::core::result::Result<T, Error>;
