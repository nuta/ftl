#![allow(non_camel_case_types)]

pub mod asm;
pub mod errno;
pub mod sys;

pub type c_int = i32;
pub type c_long = isize;
pub type c_ulong = usize;
pub type c_void = core::ffi::c_void;
pub type size_t = usize;
