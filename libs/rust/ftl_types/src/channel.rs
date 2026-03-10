use crate::error::ErrorCode;
use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const ERROR_REPLY: Self = Self::new(1, false, 0, 0, size_of::<ErrorReplyInline>());
    pub const OPEN: Self = Self::new(2, true, 0, 1, size_of::<OpenInline>());
    pub const OPEN_REPLY: Self = Self::new(3, false, 1, 0, size_of::<OpenReplyInline>());
    pub const READ: Self = Self::new(4, true, 0, 1, size_of::<ReadInline>());
    pub const READ_REPLY: Self = Self::new(5, false, 0, 0, size_of::<ReadReplyInline>());
    pub const WRITE: Self = Self::new(6, true, 0, 1, size_of::<WriteInline>());
    pub const WRITE_REPLY: Self = Self::new(7, false, 0, 0, size_of::<WriteReplyInline>());
    pub const READ_URI: Self = Self::new(8, true, 0, 2, size_of::<ReadUriInline>());
    pub const READ_URI_REPLY: Self = Self::new(9, false, 0, 0, size_of::<ReadUriReplyInline>());
    pub const WRITE_URI: Self = Self::new(10, true, 0, 2, size_of::<WriteUriInline>());
    pub const WRITE_URI_REPLY: Self = Self::new(11, false, 0, 0, size_of::<WriteUriReplyInline>());

    const fn new(
        kind: u32,
        is_call: bool,
        num_handles: u32,
        num_ools: u32,
        inline_len: usize,
    ) -> Self {
        debug_assert!(kind <= 0b11111);
        debug_assert!(num_handles <= NUM_HANDLES_MAX as u32);
        debug_assert!(num_ools <= NUM_OOLS_MAX as u32);
        debug_assert!(inline_len <= INLINE_LEN_MAX);
        Self(
            (kind << 13)
                | ((is_call as u32) << 12)
                | (num_handles << 10 | (num_ools << 8) | (inline_len as u32)),
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

    pub const fn num_handles(self) -> usize {
        ((self.0 >> 10) & 0b11) as usize
    }

    pub const fn num_ools(self) -> usize {
        ((self.0 >> 8) & 0b11) as usize
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

pub const NUM_HANDLES_MAX: usize = 2;
pub const NUM_OOLS_MAX: usize = 2;
pub const INLINE_LEN_MAX: usize =
    128 - size_of::<[HandleId; NUM_HANDLES_MAX]>() - size_of::<[OutOfLine; NUM_OOLS_MAX]>();

#[derive(Clone, Copy)]
#[repr(C)]
pub struct OutOfLine {
    pub addr: usize,
    pub len: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MessageBody {
    pub handles: [HandleId; NUM_HANDLES_MAX],
    pub ools: [OutOfLine; NUM_OOLS_MAX],
    pub inline: [u8; INLINE_LEN_MAX],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union MessageInlineBody {
    pub open: OpenInline,
    pub read: ReadInline,
    pub write: WriteInline,
    pub read_uri: ReadUriInline,
    pub write_uri: WriteUriInline,
    pub read_reply: ReadReplyInline,
    pub write_reply: WriteReplyInline,
    pub read_uri_reply: ReadUriReplyInline,
    pub write_uri_reply: WriteUriReplyInline,
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
pub struct ReadUriInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ReadUriReplyInline {
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WriteUriInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WriteUriReplyInline {
    pub len: usize,
}
