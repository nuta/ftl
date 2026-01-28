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
}

impl From<usize> for ErrorCode {
    fn from(value: usize) -> Self {
        // TODO: Optimize this conversion.
        match value {
            1 => Self::Unreachable,
            2 => Self::OutOfMemory,
            3 => Self::OutOfBounds,
            4 => Self::UnknownSyscall,
            _ => Self::Unknown,
        }
    }
}
