/// Kernel error codes.
///
/// All variants must be negative so that system call errors can be
/// distinguished from success.
#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
#[repr(i8)]
pub enum ErrorCode {
    OutOfMemory = -1,
    NotAllowed = -2,
    AlreadyExists = -3,
    InvalidArg = -4,
    InvalidState = -5,
    InvalidType = -6,
    OutOfBounds = -7,
    PageFault = -8,
    Unsupported = -9,
    TooManyHandles = -10,
    NoRoute = -11,
    Empty = -12,
    NotFound = -13,
    // This must be the last variant.
    BadErrorCode = -14,
}

impl ErrorCode {
    pub const fn from_usize(raw: usize) -> Self {
        let raw = raw as i8;
        if raw > Self::OutOfMemory as i8 || raw <= Self::BadErrorCode as i8 {
            return Self::BadErrorCode;
        }

        // SAFETY: The range is checked above.
        unsafe { core::mem::transmute(raw) }
    }

    pub const fn as_usize(self) -> usize {
        self as usize
    }
}
