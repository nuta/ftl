use ftl::warn;
use ftl_types::error::ErrorCode;

use super::c_int;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Errno(c_int);

impl Errno {
    pub const EPERM: Self = Self(1);
    pub const ENOENT: Self = Self(2);
    pub const ESRCH: Self = Self(3);
    pub const EINTR: Self = Self(4);
    pub const EBADF: Self = Self(9);
    pub const ECHILD: Self = Self(10);
    pub const EAGAIN: Self = Self(11);
    pub const EFAULT: Self = Self(14);
    pub const EBUSY: Self = Self(16);
    pub const EEXIST: Self = Self(17);
    pub const ENOMEM: Self = Self(12);
    pub const EMFILE: Self = Self(24);
    pub const EINVAL: Self = Self(22);
    pub const ESPIPE: Self = Self(29);
    pub const ENOSYS: Self = Self(38);
    pub const ENOTSOCK: Self = Self(88);
    pub const ENOTSUP: Self = Self(95);
    pub const ENOTCONN: Self = Self(107);

    pub const fn as_int(self) -> c_int {
        self.0
    }
}

impl From<ErrorCode> for Errno {
    fn from(error: ErrorCode) -> Self {
        match error {
            ErrorCode::OutOfMemory => Self::ENOMEM,
            ErrorCode::NotAllowed => Self::EPERM,
            ErrorCode::AlreadyExists | ErrorCode::AlreadyMapped => Self::EEXIST,
            ErrorCode::InvalidState
            | ErrorCode::ThreadNotRunnable
            | ErrorCode::ThreadAlreadyStarted => Self::EBUSY,
            ErrorCode::PageFault => Self::EFAULT,
            ErrorCode::Unsupported => Self::ENOTSUP,
            ErrorCode::UnknownSyscall => Self::ENOSYS,
            ErrorCode::TooManyHandles => Self::EMFILE,
            ErrorCode::HandleNotFound => Self::EBADF,
            ErrorCode::InvalidHandleId
            | ErrorCode::InvalidHandleType
            | ErrorCode::InvalidArg
            | ErrorCode::InvalidType
            | ErrorCode::InvalidUserAddr
            | ErrorCode::InvalidPageAttrs
            | ErrorCode::NotAligned
            | ErrorCode::PacketTooLong
            | ErrorCode::OutOfBounds => Self::EINVAL,
            // TODO: better errno
            _ => {
                warn!("unmapped error code: {:?}", error);
                Self::EINVAL
            }
        }
    }
}
