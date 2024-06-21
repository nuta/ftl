#![no_std]

mod inlinedstring;
mod inlinedvec;

pub use inlinedstring::InlinedString;
pub use inlinedvec::ExceedsCapacityError;
pub use inlinedvec::InlinedVec;
