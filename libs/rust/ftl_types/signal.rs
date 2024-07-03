use core::ops;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct SignalBits(u32);

impl SignalBits {
    pub const fn empty() -> SignalBits {
        SignalBits(0)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn clear(&mut self) -> SignalBits {
        let old = *self;
        self.0 = 0;
        old
    }
}

impl ops::BitOrAssign for SignalBits {
    fn bitor_assign(&mut self, rhs: SignalBits) {
        self.0 |= rhs.0;
    }
}
