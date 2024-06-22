use core::mem::size_of;

use crate::handle::HandleId;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MessageInfo(isize);

impl MessageInfo {
    pub fn raw(&self) -> isize {
        self.0
    }
}

const MESSAGE_DATA_MAX_LEN: usize = 4096 - 4 * size_of::<HandleId>();

pub struct MessageBuffer {
    pub handles: [HandleId; 4],
    pub data: [u8; MESSAGE_DATA_MAX_LEN],
}
