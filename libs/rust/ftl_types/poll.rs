use core::ops;

use crate::handle::HandleId;
use crate::handle::HANDLE_ID_BITS;
use crate::handle::HANDLE_ID_MASK;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct PollEvent(u8);

impl PollEvent {
    pub const READABLE: PollEvent = PollEvent(1 << 0);
    pub const WRITABLE: PollEvent = PollEvent(1 << 1);

    pub fn zeroed() -> PollEvent {
        PollEvent(0)
    }

    pub fn is_readable(&self) -> bool {
        self.0 & Self::READABLE.0 != 0
    }

    pub fn is_writable(&self) -> bool {
        self.0 & Self::WRITABLE.0 != 0
    }
}

impl ops::BitOr for PollEvent {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        PollEvent(self.0 | rhs.0)
    }
}

impl ops::BitOrAssign for PollEvent {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

pub struct PollSyscallResult(isize);

impl PollSyscallResult {
    pub const fn from_raw(value: isize) -> PollSyscallResult {
        PollSyscallResult(value)
    }

    pub fn event(&self) -> PollEvent {
        PollEvent((self.0 >> HANDLE_ID_BITS) as u8)
    }

    pub fn handle(&self) -> HandleId {
        HandleId::from_raw_isize_truncated(self.0 & (HANDLE_ID_MASK as isize))
    }
}
