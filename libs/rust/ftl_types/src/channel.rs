use crate::error::ErrorCode;
use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const ERROR_REPLY: Self = Self::new(1, 0, 0, size_of::<ErrorReplyInline>());
    pub const OPEN: Self = Self::new(2, 0, 1, size_of::<OpenInline>());
    pub const OPEN_REPLY: Self = Self::new(3, 1, 0, size_of::<OpenReplyInline>());
    pub const READ: Self = Self::new(4, 0, 1, size_of::<ReadInline>());
    pub const READ_REPLY: Self = Self::new(5, 0, 0, size_of::<ReadReplyInline>());
    pub const WRITE: Self = Self::new(6, 0, 1, size_of::<WriteInline>());
    pub const WRITE_REPLY: Self = Self::new(7, 0, 0, size_of::<WriteReplyInline>());

    const fn new(kind: u32, num_handles: u32, num_ools: u32, inline_len: usize) -> Self {
        debug_assert!(kind < 0b1111);
        debug_assert!(num_handles <= NUM_HANDLES_MAX as u32);
        debug_assert!(num_ools <= NUM_OOLS_MAX as u32);
        debug_assert!(inline_len <= INLINE_LEN_MAX);
        Self((kind << 12) | (num_handles << 10 | (num_ools << 8) | (inline_len as u32)))
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn kind(self) -> u32 {
        (self.0 >> 12) & 0b1111
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
pub struct ErrorReplyInline {
    pub error: ErrorCode,
}

#[repr(C)]
pub struct OpenInline {}

#[repr(C)]
pub struct OpenReplyInline {}

#[repr(C)]
pub struct ReadInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
pub struct ReadReplyInline {
    pub len: usize,
}

#[repr(C)]
pub struct WriteInline {
    pub offset: usize,
    pub len: usize,
}

#[repr(C)]
pub struct WriteReplyInline {
    pub len: usize,
}
