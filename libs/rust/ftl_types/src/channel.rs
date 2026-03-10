use crate::error::ErrorCode;
use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const ERROR_REPLY: Self = Self::new(1, false, false, false, size_of::<ErrorReplyInline>());
    pub const OPEN: Self = Self::new(2, true, false, true, size_of::<OpenInline>());
    pub const OPEN_REPLY: Self = Self::new(3, false, true, false, size_of::<OpenReplyInline>());
    pub const READ: Self = Self::new(4, true, false, true, size_of::<ReadInline>());
    pub const READ_REPLY: Self = Self::new(5, false, false, false, size_of::<ReadReplyInline>());
    pub const WRITE: Self = Self::new(6, true, false, true, size_of::<WriteInline>());
    pub const WRITE_REPLY: Self = Self::new(7, false, false, false, size_of::<WriteReplyInline>());
    pub const GETATTR: Self = Self::new(8, true, false, true, size_of::<GetattrInline>());
    pub const GETATTR_REPLY: Self =
        Self::new(9, false, false, false, size_of::<GetattrReplyInline>());
    pub const SETATTR: Self = Self::new(10, true, false, true, size_of::<SetattrInline>());
    pub const SETATTR_REPLY: Self =
        Self::new(11, false, false, false, size_of::<SetattrReplyInline>());

    const fn new(kind: u32, is_call: bool, handle: bool, ool: bool, inline_len: usize) -> Self {
        debug_assert!(kind <= 0b11111);
        debug_assert!(inline_len <= INLINE_LEN_MAX);
        Self(
            (kind << 13)
                | ((is_call as u32) << 12)
                | ((handle as u32) << 9 | (ool as u32) << 8 | (inline_len as u32)),
        )
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn is_call(self) -> bool {
        (self.0 >> 12) & 0b1 == 1
    }

    pub const fn kind(self) -> u32 {
        (self.0 >> 13) & 0b1_1111
    }

    pub const fn contains_handle(self) -> bool {
        ((self.0 >> 9) & 1) != 0
    }

    pub const fn contains_ool(self) -> bool {
        ((self.0 >> 8) & 1) != 0
    }

    pub const fn inline_len(self) -> usize {
        (self.0 & 0xff) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallId(u32);

impl CallId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

pub const INLINE_LEN_MAX: usize = 128;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct OutOfLine {
    pub addr: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attr(u32);

impl Attr {
    pub const MAC: Self = Self::from_str("mac");

    const fn from_str(s: &'static str) -> Self {
        assert!(s.len() <= 4);
        let mut attrs = 0;
        let mut i = 0;
        while i < s.len() {
            attrs |= (s.as_bytes()[i] as u32) << (i * 8);
            i += 1;
        }
        Self(attrs)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MessageBody {
    pub handle: HandleId,
    pub ool_addr: usize,
    pub ool_len: usize,
    pub inline: MessageInlineBody,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union MessageInlineBody {
    pub raw: [u8; INLINE_LEN_MAX],
    pub open: OpenInline,
    pub read: ReadInline,
    pub write: WriteInline,
    pub getattr: GetattrInline,
    pub setattr: SetattrInline,
    pub read_reply: ReadReplyInline,
    pub write_reply: WriteReplyInline,
    pub getattr_reply: GetattrReplyInline,
    pub setattr_reply: SetattrReplyInline,
    pub error_reply: ErrorReplyInline,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ErrorReplyInline {
    pub error: ErrorCode,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenInline {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenReplyInline {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ReadInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ReadReplyInline {
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WriteInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WriteReplyInline {
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GetattrInline {
    pub attr: Attr,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GetattrReplyInline {
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SetattrInline {
    pub attr: Attr,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SetattrReplyInline {
    pub len: usize,
}
