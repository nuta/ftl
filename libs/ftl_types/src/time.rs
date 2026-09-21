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

/// A wall time, the clock on your room's wall.
///
/// This struct represents the nanoseconds elpased since 1970-01-01 00:00:00,
/// excluding leap seconds, in UTC. Also known as *Unix time*.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct WallTime(u64);

impl WallTime {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    pub const fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Constructs a wall time from UTC date/time.
    ///
    /// | Param   | Valid range (inclusive)  |
    /// |---------|--------------------------|
    /// | `year`  | `[1970, 2554]`           |
    /// | `month` | `[1, 12]`                |
    /// | `day`   | `[1, last_day_of_month]` |
    /// | `hour`  | `[0, 23]`                |
    /// | `min`   | `[0, 59]`                |
    /// | `sec`   | `[0, 59]`                |
    pub const fn from_utc(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        min: u8,
        sec: u8,
    ) -> Option<Self> {
        if !matches!(year, 1970..=2554)
            || !matches!(month, 1..=12)
            || !matches!(hour, 0..=23)
            || !matches!(min, 0..=59)
            || !matches!(sec, 0..=59)
        {
            return None;
        }

        if !(1 <= day && day <= last_day_of_month(year, month)) {
            return None;
        }

        let days = days_from_civil(year as i64, month as u32, day as u32);

        let Some(secs) = (days as u64).checked_mul(86_400) else {
            return None;
        };
        let Some(secs) = secs.checked_add(hour as u64 * 3_600) else {
            return None;
        };
        let Some(secs) = secs.checked_add(min as u64 * 60) else {
            return None;
        };
        let Some(secs) = secs.checked_add(sec as u64) else {
            return None;
        };
        let Some(nanos) = secs.checked_mul(1_000_000_000) else {
            return None;
        };
        Some(Self::from_nanos(nanos))
    }
}

/// Returns days since 1970-01-01.
///
/// # Attribution
///
/// Based on [Howard Hinnant's implementation](https://howardhinnant.github.io/date_algorithms.html)
/// which is in the public domain ("Consider these donated to the public domain.").
const fn days_from_civil(mut y: i64, m: u32, d: u32) -> i64 {
    y -= (m <= 2) as i64;
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe as i64 - 719468
}

/// Returns true if the given year is a leap year.
///
/// # Original implementation
///
/// Based on [Howard Hinnant's implementation](https://howardhinnant.github.io/date_algorithms.html)
/// which is in the public domain ("Consider these donated to the public domain.").
const fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Returns the last day of the month for a common year.
///
/// # Attribution
///
/// Based on [Howard Hinnant's implementation](https://howardhinnant.github.io/date_algorithms.html)
/// which is in the public domain ("Consider these donated to the public domain.").
const fn last_day_of_month_common_year(m: u8) -> u8 {
    const A: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    A[(m - 1) as usize]
}

/// Returns the last day of the month for a given year.
///
/// # Attribution
///
/// Based on [Howard Hinnant's implementation](https://howardhinnant.github.io/date_algorithms.html)
/// which is in the public domain ("Consider these donated to the public domain.").
const fn last_day_of_month(y: i32, m: u8) -> u8 {
    if m != 2 || !is_leap(y) {
        last_day_of_month_common_year(m)
    } else {
        29
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
