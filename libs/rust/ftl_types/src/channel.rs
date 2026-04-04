use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageKind(u32);

impl MessageKind {
    // Note: Reply messages must have odd numbers.
    pub const ERROR_REPLY: Self = Self::new(1);
    pub const OPEN: Self = Self::new(2).with_body();
    pub const OPEN_REPLY: Self = Self::new(3).with_handle();
    pub const READ: Self = Self::new(4).with_body();
    pub const READ_REPLY: Self = Self::new(5);
    pub const WRITE: Self = Self::new(6).with_body();
    pub const WRITE_REPLY: Self = Self::new(7);
    pub const GETATTR: Self = Self::new(8).with_body();
    pub const GETATTR_REPLY: Self = Self::new(9);
    pub const SETATTR: Self = Self::new(10).with_body();
    pub const SETATTR_REPLY: Self = Self::new(11);

    const fn new(kind: u8) -> Self {
        debug_assert!(kind <= 0b1111);
        Self((kind as u32) << 28)
    }

    const fn with_body(self) -> Self {
        Self(self.0 | 1 << 26)
    }

    const fn with_handle(self) -> Self {
        Self(self.0 | 1 << 27)
    }
}

impl fmt::Debug for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind_str = match *self {
            Self::ERROR_REPLY => "ERROR_REPLY",
            Self::OPEN => "OPEN",
            Self::OPEN_REPLY => "OPEN_REPLY",
            Self::READ => "READ",
            Self::READ_REPLY => "READ_REPLY",
            Self::WRITE => "WRITE",
            Self::WRITE_REPLY => "WRITE_REPLY",
            Self::GETATTR => "GETATTR",
            Self::GETATTR_REPLY => "GETATTR_REPLY",
            Self::SETATTR => "SETATTR",
            Self::SETATTR_REPLY => "SETATTR_REPLY",
            _ => return write!(f, "Unknown({:#x})", self.0),
        };
        write!(f, "{}", kind_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const fn new(kind: MessageKind, mid: MessageId, body_len: usize) -> Self {
        assert!(body_len <= 0x3fff); // 14 bits
        let mut raw = 0;
        raw |= kind.0 as u32;
        raw |= (mid.0 as u32) << 14;
        raw |= body_len as u32;
        Self(raw)
    }

    pub const fn as_raw(self) -> usize {
        self.0 as usize
    }

    pub const fn from_raw(raw: usize) -> Self {
        Self(raw as u32)
    }

    pub const fn kind(self) -> MessageKind {
        MessageKind(self.0 & 0xfc000000)
    }

    pub const fn mid(self) -> MessageId {
        MessageId(((self.0 >> 14) & 0xfff) as u16)
    }

    pub const fn has_body(self) -> bool {
        self.0 & (1 << 26) != 0
    }

    pub const fn has_handle(self) -> bool {
        self.0 & (1 << 27) != 0
    }

    pub const fn body_len(self) -> usize {
        (self.0 & 0x3fff) as usize
    }

    pub const fn is_reply(self) -> bool {
        let raw = self.kind().0 >> 28;
        raw % 2 == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(u16);

impl MessageId {
    pub const fn new(id: u16) -> Self {
        assert!(id <= 0xfff); // 12 bits
        Self(id)
    }

    pub const fn as_u16(self) -> u16 {
        self.0
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpenOptions(u32);

impl OpenOptions {
    /// Connect mode: connect to a service, TCP socket, etc.
    pub const CONNECT: Self = Self::new(1);
    /// Listen mode: listen for connections on a TCP socket, etc.
    pub const LISTEN: Self = Self::new(2);

    const fn new(kind: u32) -> Self {
        Self(kind)
    }

    pub fn from_usize(value: usize) -> Self {
        Self(value as u32)
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}
