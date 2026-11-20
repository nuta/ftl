use crate::types::c_void;
use crate::types::size_t;

#[repr(C)]
pub struct IoVec {
    pub iov_base: *mut c_void,
    pub iov_len: size_t,
}
