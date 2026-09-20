#![allow(non_camel_case_types)]

pub mod asm;
pub mod errno;
pub mod signal;
pub mod sys;
pub mod unistd;

pub type c_int = i32;
pub type c_short = i16;
pub type c_unsigned = u32;
pub type c_long = isize;
pub type c_ulong = usize;
pub type c_void = core::ffi::c_void;
pub type size_t = usize;
pub type off_t = i64;
