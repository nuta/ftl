#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FtlError {
    AlreadyExists = -1,
    ClosedByPeer = -2,
    Empty = -3,
    Full = -4,
}
