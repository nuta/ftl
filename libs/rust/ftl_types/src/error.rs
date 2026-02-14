/// The error code.
///
/// Note: Do not change the size of this enum: ERROR_RETVAL_BASE assumes this
/// enum is 8 bits wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorCode {
    Unknown = 0,
    Unreachable = 1,
    OutOfMemory = 2,
    OutOfBounds = 3,
    UnknownSyscall = 4,
    Unsupported = 5,
    InvalidArgument = 6,
    NotAllowed = 7,
    TooManyHandles = 8,
    HandleNotFound = 9,
    InvalidHandle = 10,
    PeerClosed = 11,
    InvalidMessage = 12,
    AlreadyExists = 13,
    NotFound = 14,
    TryLater = 15,
}

impl From<usize> for ErrorCode {
    fn from(value: usize) -> Self {
        // TODO: Optimize this conversion.
        match value {
            1 => Self::Unreachable,
            2 => Self::OutOfMemory,
            3 => Self::OutOfBounds,
            4 => Self::UnknownSyscall,
            5 => Self::Unsupported,
            6 => Self::InvalidArgument,
            7 => Self::NotAllowed,
            8 => Self::TooManyHandles,
            9 => Self::HandleNotFound,
            10 => Self::InvalidHandle,
            11 => Self::PeerClosed,
            12 => Self::InvalidMessage,
            13 => Self::AlreadyExists,
            14 => Self::NotFound,
            15 => Self::TryLater,
            _ => Self::Unknown,
        }
    }
}
