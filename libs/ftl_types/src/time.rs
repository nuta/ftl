use core::ops::Add;

// Note: MonoTime wraps at 2^64.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct MonoTime(u64);

impl MonoTime {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Returns the elapsed duration, or None if `earlier` is later than this
    /// time.
    ///
    /// This assumes the duration between two MonoTimes is less than 2^63
    /// nanoseconds (292 years - restart your computers by then).
    pub const fn duration_since(&self, earlier: Self) -> Option<Duration> {
        let nanos = self.0.wrapping_sub(earlier.0);
        if (nanos as i64) >= 0 {
            Some(Duration::from_nanos(nanos))
        } else {
            None
        }
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

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000_000)
    }

    pub const fn from_secs(secs: u64) -> Self {
        Self(secs * 1_000_000_000)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }
}
