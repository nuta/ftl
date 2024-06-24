use core::mem::size_of;

use crate::handle::HandleId;

/// The message metadata.
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

    pub const fn message_id(self) -> isize {
        self.0 >> 14
    }

    pub const fn num_handles(self) -> usize {
        self.0 as usize >> 12 & 0b11
    }

    pub const fn data_len(self) -> usize {
        // FIXME:
        debug_assert!(self.0 & 0xffff < MESSAGE_DATA_MAX_LEN as isize);

        (self.0 & 0xffff) as usize
    }
}

pub const MESSAGE_DATA_MAX_LEN: usize = 4096 - 4 * size_of::<HandleId>();

#[repr(C, align(16))] // Don't reorder fields
pub struct MessageBuffer {
    pub handles: [HandleId; 4],
    pub data: [u8; MESSAGE_DATA_MAX_LEN],
}
