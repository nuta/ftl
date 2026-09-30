use crate::types::c_int;

pub const FUTEX_WAIT: c_int = 0;
pub const FUTEX_WAKE: c_int = 1;
pub const FUTEX_WAIT_BITSET: c_int = 9;
pub const FUTEX_WAKE_BITSET: c_int = 10;
pub const FUTEX_PRIVATE_FLAG: c_int = 128;
pub const FUTEX_CLOCK_REALTIME: c_int = 256;
pub const FUTEX_BITSET_MATCH_ANY: u32 = 0xffff_ffff;
