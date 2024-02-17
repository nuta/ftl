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
    pub const fn zeroed() -> SignalSet {
        SignalSet { signals: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.signals == 0
    }

    pub fn add(&mut self, signal: Signal) {
        self.signals |= signal as u32;
    }

    /// Clears all signals and returns the old value.
    pub fn clear(&mut self) -> SignalSet {
        let old = self.signals;
        self.signals = 0;
        SignalSet { signals: old }
    }

    pub fn pop(&mut self) -> Option<Signal> {
        if self.signals == 0 {
            return None;
        }

        let signal = self.signals.trailing_zeros();
        self.signals &= !(1 << signal);
        match signal {
            0 => Some(Signal::Interrupt),
            _ => todo!(),
        }
    }
}
