pub const RLIMIT_STACK: u32 = 3;
pub const RLIMIT_NOFILE: u32 = 7;
pub const RLIM_INFINITY: u64 = u64::MAX;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}
