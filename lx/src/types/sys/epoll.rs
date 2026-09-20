use crate::types::c_int;
use crate::types::sys::fcntl::O_CLOEXEC;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

pub const EPOLL_CTL_ADD: c_int = 1;
pub const EPOLL_CTL_DEL: c_int = 2;
pub const EPOLL_CTL_MOD: c_int = 3;
pub const EPOLL_CLOEXEC: c_int = O_CLOEXEC;
pub const EPOLLET: u32 = 1 << 31;
