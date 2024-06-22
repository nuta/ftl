use core::mem::size_of;

use crate::handle::HandleId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct MessageInfo(isize);

impl MessageInfo {
    pub const fn from_raw(raw: isize) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> isize {
        self.0
    }

    pub const fn data_len(self) -> usize {
        (self.0 & 0xffff) as usize
    }
}

const MESSAGE_DATA_MAX_LEN: usize = 4096 - 4 * size_of::<HandleId>();

pub struct MessageBuffer {
    pub handles: [HandleId; 4],
    pub data: [u8; MESSAGE_DATA_MAX_LEN],
}
