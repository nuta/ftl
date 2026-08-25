use crate::types::c_void;
use crate::types::size_t;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct iovec {
    pub iov_base: *mut c_void,
    pub iov_len: size_t,
}
