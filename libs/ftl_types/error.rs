#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FtlError {
    OutOfMemory = -1,
    ClosedByPeer = -2,
    InvalidParams = -3,
}
