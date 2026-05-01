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
    pub const fn from_nanos(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn as_raw(&self) -> u64 {
        self.0
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
        diff != 0 && diff <= SAFE_DELTA
    }
}
