use core::ops::Add;
use core::time::Duration;

/// The [`std::time::Instant`] for FTL.
///
/// This is a monotonic clock used only for relative time measurements of two
/// instant types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Monotonic(u64);

impl Monotonic {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub const fn from_micros(micros: u64) -> Self {
        Self(micros * 1_000)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000_000)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }

    pub const fn as_micros(&self) -> u64 {
        self.0 / 1_000
    }

    pub const fn as_millis(&self) -> u64 {
        self.0 / 1_000_000
    }

    pub fn elapsed_since(&self, other: &Self) -> Duration {
        //
        Duration::from_nanos(self.0.wrapping_sub(other.0))
    }

    /// Compare two tick values considering potential wrapping.
    ///
    /// Returns true if `a` is before `b` in circular time. This works correctly
    /// even when the tick counter wraps around.
    pub fn is_before(&self, other: &Self) -> bool {
        // If the difference is less than the maximum timer duration, self is before
        // other.
        self.0.wrapping_sub(other.0) < (u64::MAX / 2)
    }
}

impl Add<Duration> for Monotonic {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        // FIXME: u64
        Self(self.0 + rhs.as_nanos() as u64)
    }
}
