use crate::types::c_int;
use crate::types::c_short;
use crate::types::c_unsigned;

pub type nfds_t = c_unsigned;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PollFd {
    pub fd: c_int,
    pub events: c_short,
    pub revents: c_short,
}

pub const POLLIN: c_short = 0x0001;
pub const POLLOUT: c_short = 0x0004;
pub const POLLNVAL: c_short = 0x0020;
