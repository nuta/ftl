/// The error code.
///
/// Note: Do not change the size of this enum: ERROR_RETVAL_BASE assumes this
/// enum is 8 bits wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorCode {
    Unknown,
    Unreachable,
    OutOfMemory,
    OutOfBounds,
    UnknownSyscall,
}

impl From<usize> for ErrorCode {
    fn from(value: usize) -> Self {
        // TODO: Optimize this conversion.
        match value {
            1 => Self::OutOfMemory,
            2 => Self::OutOfBounds,
            3 => Self::UnknownSyscall,
            _ => Self::Unknown,
        }
    }
}
