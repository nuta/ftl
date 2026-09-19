use crate::types::c_int;
use crate::types::sys::fcntl::O_NONBLOCK;

pub const EFD_SEMAPHORE: c_int = 1;
pub const EFD_NONBLOCK: c_int = O_NONBLOCK;
