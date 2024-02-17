use core::ops::BitAnd;
use core::ops::BitAndAssign;
use core::ops::BitOr;
use core::ops::BitOrAssign;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    bits: u32,
}

impl Event {
    pub const READABLE: Event = Event { bits: 1 << 0 };
    pub const WRITABLE: Event = Event { bits: 1 << 1 };

    pub fn zeroed() -> Event {
        Event { bits: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }
}

impl BitOr for Event {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Event {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOrAssign for Event {
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl BitAnd for Event {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Event {
            bits: self.bits & rhs.bits,
        }
    }
}

impl BitAndAssign for Event {
    fn bitand_assign(&mut self, rhs: Self) {
        self.bits &= rhs.bits;
    }
}
