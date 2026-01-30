pub const MSGTYPE_ERROR_REPLY: u8 = 1;
pub const MSGTYPE_READ: u8 = 2;
pub const MSGTYPE_READ_REPLY: u8 = 3;
pub const MSGTYPE_WRITE: u8 = 4;
pub const MSGTYPE_WRITE_REPLY: u8 = 5;
pub const MSGTYPE_OPEN: u8 = 6;
pub const MSGTYPE_OPEN_REPLY: u8 = 7;

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
