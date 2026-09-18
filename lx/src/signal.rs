use crate::types;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::signal::NSIG;
use crate::types::signal::SIG_DFL;
use crate::types::signal::SIG_IGN;
use crate::types::signal::SIGKILL;
use crate::types::signal::SIGSTOP;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signal(u8);

impl Signal {
    pub const KILL: Self = Self(SIGKILL as u8);
    pub const STOP: Self = Self(SIGSTOP as u8);

    pub fn from_raw(raw: c_int) -> Result<Self, Errno> {
        if raw <= 0 || raw >= NSIG {
            return Err(Errno::EINVAL);
        }

        Ok(Self(raw as u8))
    }

    pub fn number(self) -> c_int {
        self.0.into()
    }

    pub fn is_uncatchable(self) -> bool {
        self == Self::KILL || self == Self::STOP
    }

    fn index(self) -> usize {
        usize::from(self.0 - 1)
    }

    fn bit(self) -> u64 {
        1 << self.index()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct SignalSet(u64);

impl SignalSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn to_raw(self) -> u64 {
        self.0
    }

    pub fn insert(&mut self, signal: Signal) {
        self.0 |= signal.bit();
    }

    pub fn remove(&mut self, signal: Signal) {
        self.0 &= !signal.bit();
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn take_first(&mut self) -> Option<Signal> {
        if self.is_empty() {
            return None;
        }

        let index = self.0.trailing_zeros() as u8;
        let signal = Signal(index + 1);
        self.remove(signal);
        Some(signal)
    }
}

pub struct SignalMap<T> {
    actions: [T; NSIG as usize - 1],
}

impl<T: Copy> SignalMap<T> {
    pub const fn new(default: T) -> Self {
        Self {
            actions: [default; NSIG as usize - 1],
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            actions: self.actions,
        }
    }
}

impl<T> SignalMap<T> {
    pub fn get(&self, signal: Signal) -> &T {
        &self.actions[signal.index()]
    }

    pub fn set(&mut self, signal: Signal, action: T) {
        self.actions[signal.index()] = action;
    }
}

#[derive(Clone, Copy)]
pub enum SigDisposition {
    Default,
    Ignore,
    Handler(usize),
}

#[derive(Clone, Copy)]
pub struct SigAction {
    disposition: SigDisposition,
    flags: usize,
    restorer: usize,
    mask: SignalSet,
}

impl SigAction {
    pub const fn default() -> Self {
        Self {
            disposition: SigDisposition::Default,
            flags: 0,
            restorer: 0,
            mask: SignalSet::empty(),
        }
    }

    pub fn from_raw(action: types::signal::SigAction) -> Self {
        let disposition = match action.handler {
            SIG_DFL => SigDisposition::Default,
            SIG_IGN => SigDisposition::Ignore,
            handler => SigDisposition::Handler(handler),
        };

        Self {
            disposition,
            flags: action.flags,
            restorer: action.restorer,
            mask: SignalSet::from_raw(action.mask),
        }
    }

    pub fn to_raw(self) -> types::signal::SigAction {
        let handler = match self.disposition {
            SigDisposition::Default => SIG_DFL,
            SigDisposition::Ignore => SIG_IGN,
            SigDisposition::Handler(handler) => handler,
        };

        types::signal::SigAction {
            handler,
            flags: self.flags,
            restorer: self.restorer,
            mask: self.mask.to_raw(),
        }
    }

    pub fn disposition(self) -> SigDisposition {
        self.disposition
    }

    pub fn restorer(self) -> usize {
        self.restorer
    }
}
