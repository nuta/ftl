#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Interrupt = 1 << 0,
}

#[derive(Debug)]
pub struct SignalSet {
    pub signals: u32,
}

impl SignalSet {
    pub const fn empty() -> SignalSet {
        SignalSet { signals: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.signals == 0
    }

    pub fn add(&mut self, signal: Signal) {
        self.signals |= signal as u32;
    }

    pub fn add_set(&mut self, other: SignalSet) {
        self.signals |= other.signals;
    }

    /// Clears all signals and returns the old value.
    pub fn clear(&mut self) -> SignalSet {
        let old = self.signals;
        self.signals = 0;
        SignalSet { signals: old }
    }
}
