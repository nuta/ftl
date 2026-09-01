use crate::types::c_int;

pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_NONBLOCK: c_int = 0o4000;
