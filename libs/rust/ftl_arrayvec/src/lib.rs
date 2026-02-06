#![no_std]

mod array_string;
mod array_vec;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CapacityError;

pub use array_string::ArrayString;
pub use array_vec::ArrayVec;
