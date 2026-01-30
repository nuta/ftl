use crate::error::ErrorCode;
use crate::handle::HandleId;

pub const MSGTYPE_ERROR_REPLY: u8 = 1;
pub const MSGTYPE_OPEN: u8 = 2;
pub const MSGTYPE_OPEN_REPLY: u8 = 3;
pub const MSGTYPE_READ: u8 = 4;
pub const MSGTYPE_READ_REPLY: u8 = 5;
pub const MSGTYPE_WRITE: u8 = 6;
pub const MSGTYPE_WRITE_REPLY: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const fn len(&self) -> usize {
        todo!()
    }

    pub const fn ty(&self) -> u8 {
        todo!()
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
