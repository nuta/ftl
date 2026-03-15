use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const ERROR_REPLY: Self = Self::new(1, false, false, false);
    pub const OPEN: Self = Self::new(2, true, false, true);
    pub const OPEN_REPLY: Self = Self::new(3, false, true, false);
    pub const READ: Self = Self::new(4, true, false, true);
    pub const READ_REPLY: Self = Self::new(5, false, false, false);
    pub const WRITE: Self = Self::new(6, true, false, true);
    pub const WRITE_REPLY: Self = Self::new(7, false, false, false);
    pub const GETATTR: Self = Self::new(8, true, false, true);
    pub const GETATTR_REPLY: Self = Self::new(9, false, false, false);
    pub const SETATTR: Self = Self::new(10, true, false, true);
    pub const SETATTR_REPLY: Self = Self::new(11, false, false, false);

    const fn new(kind: u32, is_call: bool, handle: bool, ool: bool) -> Self {
        debug_assert!(kind <= 0b11111);
        Self((kind << 3) | ((is_call as u32) << 2) | ((handle as u32) << 1 | (ool as u32) << 0))
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn is_call(self) -> bool {
        (self.0 >> 2) & 0b1 == 1
    }

    pub const fn kind(self) -> u32 {
        (self.0 >> 3) & 0b1_1111
    }

    pub const fn contains_handle(self) -> bool {
        ((self.0 >> 1) & 1) != 0
    }

    pub const fn contains_ool(self) -> bool {
        ((self.0 >> 0) & 1) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u32);

impl RequestId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

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

    pub fn from_usize(value: usize) -> Self {
        debug_assert!(value <= 0xffffffff);
        Self(value as u32)
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawMessage {
    pub handle: HandleId,
    pub ool_addr: usize,
    pub ool_len: usize,
    pub inline: usize,
}
