use ftl_types::time::Duration;

use crate::types::c_int;
use crate::types::c_long;

pub const CLOCK_REALTIME: c_int = 0;
pub const CLOCK_MONOTONIC: c_int = 1;

pub const TIMER_ABSTIME: c_int = 1;

pub type time_t = c_long;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}

impl TimeSpec {
    pub fn from_nanos(nanos: u64) -> Self {
        Self {
            tv_sec: (nanos / 1_000_000_000) as time_t,
            tv_nsec: (nanos % 1_000_000_000) as c_long,
        }
    }

    pub fn from_duration(duration: Duration) -> Self {
        Self::from_nanos(duration.as_nanos())
    }

    pub fn to_nanos(&self) -> Option<u64> {
        if self.tv_sec < 0 || self.tv_nsec < 0 || self.tv_nsec >= 1_000_000_000 {
            return None;
        }

        (self.tv_sec as u64)
            .checked_mul(1_000_000_000)?
            .checked_add(self.tv_nsec as u64)
    }
}
