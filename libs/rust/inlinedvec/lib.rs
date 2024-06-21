#![no_std]

mod inlinedvec;
mod inlinedstring;

pub use inlinedvec::InlinedVec;
pub use inlinedvec::ExceedsCapacityError;
pub use inlinedstring::InlinedString;
