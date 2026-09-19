use crate::types::c_int;
use crate::types::c_long;

pub const CLOCK_REALTIME: c_int = 0;
pub const CLOCK_MONOTONIC: c_int = 1;

pub type time_t = c_long;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}
