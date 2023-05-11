//! This crate provides `#[bitfields]`, an attribute macro to define bitfield structs
//! for reading/writing MMIO registers, control registers in x86-64, and CSR registers
//! in RISC-V.
//!
//! Inspired by [tock-registers](https://github.com/tock/tock/tree/master/libraries/tock-register-interface)
//! and [clap](https://github.com/clap-rs/clap). Also, coincidentally, it's similar to
//! a crosvm's internal crate named [`bit_field`](https://github.com/google/crosvm/tree/82ce7f3131ea65057bed8739adbb2b486ef0a685/bit_field).
//! Great minds think alike!
//!
//! # Examples
//!
//! ```
//! ```
#![no_std]
pub use bitfields_derive::bitfields;

pub trait BitField {
    const BITS: usize;
}

macro_rules! define_bit_type {
    ($name:ident, $bits:expr) => {
        pub struct $name;

        impl BitField for $name {
            const BITS: usize = $bits;
        }
    };
}

define_bit_type!(B1, 1);
define_bit_type!(B2, 2);
define_bit_type!(B3, 3);
define_bit_type!(B4, 4);
define_bit_type!(B5, 5);
define_bit_type!(B6, 6);
define_bit_type!(B7, 7);
define_bit_type!(B8, 8);
define_bit_type!(B9, 9);
define_bit_type!(B10, 10);
define_bit_type!(B11, 11);
define_bit_type!(B12, 12);
define_bit_type!(B13, 13);
define_bit_type!(B14, 14);
define_bit_type!(B15, 15);
define_bit_type!(B16, 16);
define_bit_type!(B17, 17);
define_bit_type!(B18, 18);
define_bit_type!(B19, 19);
define_bit_type!(B20, 20);
define_bit_type!(B21, 21);
define_bit_type!(B22, 22);
define_bit_type!(B23, 23);
define_bit_type!(B24, 24);
define_bit_type!(B25, 25);
define_bit_type!(B26, 26);
define_bit_type!(B27, 27);
define_bit_type!(B28, 28);
define_bit_type!(B29, 29);
define_bit_type!(B30, 30);
define_bit_type!(B31, 31);
define_bit_type!(B32, 32);
define_bit_type!(B33, 33);
define_bit_type!(B34, 34);
define_bit_type!(B35, 35);
define_bit_type!(B36, 36);
define_bit_type!(B37, 37);
define_bit_type!(B38, 38);
define_bit_type!(B39, 39);
define_bit_type!(B40, 40);
define_bit_type!(B41, 41);
define_bit_type!(B42, 42);
define_bit_type!(B43, 43);
define_bit_type!(B44, 44);
define_bit_type!(B45, 45);
define_bit_type!(B46, 46);
define_bit_type!(B47, 47);
define_bit_type!(B48, 48);
define_bit_type!(B49, 49);
define_bit_type!(B50, 50);
define_bit_type!(B51, 51);
define_bit_type!(B52, 52);
define_bit_type!(B53, 53);
define_bit_type!(B54, 54);
define_bit_type!(B55, 55);
define_bit_type!(B56, 56);
define_bit_type!(B57, 57);
define_bit_type!(B58, 58);
define_bit_type!(B59, 59);
define_bit_type!(B60, 60);
define_bit_type!(B61, 61);
define_bit_type!(B62, 62);
define_bit_type!(B63, 63);
define_bit_type!(B64, 64);
