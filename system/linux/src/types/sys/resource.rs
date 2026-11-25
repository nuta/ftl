use crate::types::c_long;

pub const RLIMIT_STACK: u32 = 3;
pub const RLIMIT_NOFILE: u32 = 7;
pub const RLIM_INFINITY: u64 = u64::MAX;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

#[derive(Default)]
#[repr(C)]
pub struct RUsage {
    pub ru_utime: [c_long; 2],
    pub ru_stime: [c_long; 2],
    pub ru_maxrss: c_long,
    pub ru_ixrss: c_long,
    pub ru_idrss: c_long,
    pub ru_isrss: c_long,
    pub ru_minflt: c_long,
    pub ru_majflt: c_long,
    pub ru_nswap: c_long,
    pub ru_inblock: c_long,
    pub ru_oublock: c_long,
    pub ru_msgsnd: c_long,
    pub ru_msgrcv: c_long,
    pub ru_nsignals: c_long,
    pub ru_nvcsw: c_long,
    pub ru_nivcsw: c_long,
}
