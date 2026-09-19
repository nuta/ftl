use crate::types::c_ulong;

pub const AT_PHDR: c_ulong = 3;
pub const AT_PHENT: c_ulong = 4;
pub const AT_PHNUM: c_ulong = 5;
pub const AT_PAGESZ: c_ulong = 6;
pub const AT_RANDOM: c_ulong = 25;
pub const AT_RANDOM_LEN: usize = 16;
