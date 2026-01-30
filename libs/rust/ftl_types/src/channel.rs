use crate::error::ErrorCode;
use crate::handle::HandleId;

pub const MSGTYPE_ERROR_REPLY: u8 = 1;
pub const MSGTYPE_OPEN: u8 = 2;
pub const MSGTYPE_OPEN_REPLY: u8 = 3;
pub const MSGTYPE_READ: u8 = 4;
pub const MSGTYPE_READ_REPLY: u8 = 5;
pub const MSGTYPE_WRITE: u8 = 6;
pub const MSGTYPE_WRITE_REPLY: u8 = 7;

/// A message info.
///
/// - The length of inline data (8 bits).
/// - The message type (8 bits).
/// - # of handles (2 bits).
/// - # of out-of-line entries (2 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const OPEN: Self = Self::new(1, 0, 1, 0);
    pub const OPEN_REPLY: Self = Self::new(2, 1, 0, 0);
    pub const READ: Self = Self::new(3, 0, 1, size_of::<ReadInline>());
    pub const READ_REPLY: Self = Self::new(4, 0, 0, size_of::<ReadReplyInline>());
    pub const WRITE: Self = Self::new(5, 0, 1, size_of::<WriteInline>());
    pub const WRITE_REPLY: Self = Self::new(6, 0, 0, size_of::<WriteReplyInline>());
    pub const ERROR_REPLY: Self = Self::new(7, 0, 0, size_of::<ErrorReplyInline>());

    const fn new(ty: u32, num_handles: u32, num_ools: u32, len: usize) -> Self {
        Self(len as u32 | (ty << 8) | (num_handles << 16) | (num_ools << 18))
    }

    pub const fn len(&self) -> usize {
        (self.0 & 0xff) as usize
    }

    pub const fn num_handles(&self) -> usize {
        ((self.0 >> 16) & 0b11) as usize
    }

    pub const fn num_ools(&self) -> usize {
        ((self.0 >> 18) & 0b11) as usize
    }
}

const NUM_HANDLES_MAX: usize = 2;
const NUM_OOLS_MAX: usize = 1;
const INLINE_LEN_MAX: usize =
    64 - (size_of::<HandleId>() * NUM_HANDLES_MAX + size_of::<OutOfLine>() * NUM_OOLS_MAX);

#[repr(C)]
pub struct OutOfLine {
    pub ptr: usize,
    pub len: usize,
}

#[repr(C, align(8))]
pub struct MessageBody {
    pub handles: [HandleId; NUM_HANDLES_MAX],
    pub ools: [OutOfLine; NUM_OOLS_MAX],
    pub inline: [u8; INLINE_LEN_MAX],
}

impl MessageBody {
    pub fn inline<T: Copy + Sized>(&self) -> &T {
        // TODO: Make this better
        unsafe { &*(self.inline.as_ptr() as *const T) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TxId(u32);

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ReadInline {
    pub offset: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct WriteInline {
    pub offset: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ReadReplyInline {
    pub len: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct WriteReplyInline {
    pub len: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ErrorReplyInline {
    pub error: ErrorCode,
}
