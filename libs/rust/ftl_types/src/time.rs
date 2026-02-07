use core::time::Duration;

const SAFE_DELTA: u64 = (1u64 << 63) - 1;

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

    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        let delta_nanos: u64 = duration.as_nanos().try_into().ok()?;
        if delta_nanos > SAFE_DELTA {
            return None;
        }

        Some(Self(self.0.wrapping_add(delta_nanos)))
    }

    pub fn elapsed_since(&self, other: &Self) -> Duration {
        Duration::from_nanos(self.0.wrapping_sub(other.0))
    }

    /// Compare two tick values considering potential wrapping.
    ///
    /// Returns true if `self` is before `other` in circular time. This works
    /// correctly even when the tick counter wraps around, as long as compared
    /// deltas stay within half the ring.
    pub fn is_before(&self, other: &Self) -> bool {
        let diff = other.0.wrapping_sub(self.0);
        diff != 0 && diff < SAFE_DELTA
    }

    /// Returns true if `self` is after `other` in circular time.
    pub fn is_after(&self, other: &Self) -> bool {
        !self.is_before(other)
    }
}
