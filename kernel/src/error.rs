#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidState,
    InvalidArgument,
    OutOfMemory,
    AlreadyExists,
    NotAllowed,
    Unsupported,
    OutOfBounds,
}
