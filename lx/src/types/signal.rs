use crate::types::c_int;

pub const NSIG: c_int = 65;

pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;
pub const SIGKILL: c_int = 9;
pub const SIGSTOP: c_int = 19;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigAction {
    pub handler: usize,
    pub flags: usize,
    pub restorer: usize,
    pub mask: u64,
}
