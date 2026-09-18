use crate::types::c_int;

pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const MAP_ANONYMOUS: c_int = 0x20;
