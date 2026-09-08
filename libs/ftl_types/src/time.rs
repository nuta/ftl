use core::ops::Add;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MonoTime(u64);

impl MonoTime {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }
}

impl Add<Duration> for MonoTime {
    type Output = MonoTime;

    fn add(self, other: Duration) -> MonoTime {
        MonoTime(self.0.wrapping_add(other.as_nanos()))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration(u64);

impl Duration {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub const fn from_secs(secs: u64) -> Self {
        Self(secs * 1_000_000_000)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }
}
