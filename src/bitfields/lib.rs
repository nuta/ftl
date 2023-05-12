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
//! use bitfields::{bitfields, B30};
//! use std::mem::size_of;
//!
//! #[bitfields(u32)]       // Derives Default and Debug.
//! #[derive(Copy, Clone)]  // `#[derive]` must come after `#[bitfields]`.
//! struct Stvec {
//!     #[bitfield(default=TrapMode::Direct)]
//!     mode: TrapMode,
//!     addr: B30,
//! }
//!
//! #[bitfields(bits = 2)] // Derives Copy, Clone, Debug.
//! #[derive(PartialEq)]   // `#[derive]` must come after `#[bitfields]`.
//! enum TrapMode {
//!     Direct = 0b00,
//!     Vectored = 0b01,
//! }
//!
//! assert_eq!(size_of::<Stvec>(), size_of::<u32>());
//! let mut stvec = Stvec::default();
//! assert_eq!(stvec.mode(), TrapMode::Direct);
//! assert_eq!(stvec.addr(), 0);

//! stvec.set_mode(TrapMode::Vectored);
//! stvec.set_addr(0x1234567);
//! assert_eq!(stvec.mode(), TrapMode::Vectored);
//! assert_eq!(stvec.addr(), 0x1234567);
//! ```
#![no_std]
pub use bitfields_derive::bitfields;

/// Indicates the given value is not a valid variant of the enum.
pub struct UnknownVariantErr;

pub trait BitField {
    const BITS: usize;
    type ContainerType;
}

macro_rules! define_bit_type {
    ($name:ident, $bits:literal, $container_ty:ty) => {
        pub struct $name;

        impl BitField for $name {
            const BITS: usize = $bits;
            type ContainerType = $container_ty;
        }
    };
}

define_bit_type!(B1, 1, u8);
define_bit_type!(B2, 2, u8);
define_bit_type!(B3, 3, u8);
define_bit_type!(B4, 4, u8);
define_bit_type!(B5, 5, u8);
define_bit_type!(B6, 6, u8);
define_bit_type!(B7, 7, u8);
define_bit_type!(B8, 8, u8);
define_bit_type!(B9, 9, u16);
define_bit_type!(B10, 10, u16);
define_bit_type!(B11, 11, u16);
define_bit_type!(B12, 12, u16);
define_bit_type!(B13, 13, u16);
define_bit_type!(B14, 14, u16);
define_bit_type!(B15, 15, u16);
define_bit_type!(B16, 16, u16);
define_bit_type!(B17, 17, u32);
define_bit_type!(B18, 18, u32);
define_bit_type!(B19, 19, u32);
define_bit_type!(B20, 20, u32);
define_bit_type!(B21, 21, u32);
define_bit_type!(B22, 22, u32);
define_bit_type!(B23, 23, u32);
define_bit_type!(B24, 24, u32);
define_bit_type!(B25, 25, u32);
define_bit_type!(B26, 26, u32);
define_bit_type!(B27, 27, u32);
define_bit_type!(B28, 28, u32);
define_bit_type!(B29, 29, u32);
define_bit_type!(B30, 30, u32);
define_bit_type!(B31, 31, u32);
define_bit_type!(B32, 32, u32);
define_bit_type!(B33, 33, u64);
define_bit_type!(B34, 34, u64);
define_bit_type!(B35, 35, u64);
define_bit_type!(B36, 36, u64);
define_bit_type!(B37, 37, u64);
define_bit_type!(B38, 38, u64);
define_bit_type!(B39, 39, u64);
define_bit_type!(B40, 40, u64);
define_bit_type!(B41, 41, u64);
define_bit_type!(B42, 42, u64);
define_bit_type!(B43, 43, u64);
define_bit_type!(B44, 44, u64);
define_bit_type!(B45, 45, u64);
define_bit_type!(B46, 46, u64);
define_bit_type!(B47, 47, u64);
define_bit_type!(B48, 48, u64);
define_bit_type!(B49, 49, u64);
define_bit_type!(B50, 50, u64);
define_bit_type!(B51, 51, u64);
define_bit_type!(B52, 52, u64);
define_bit_type!(B53, 53, u64);
define_bit_type!(B54, 54, u64);
define_bit_type!(B55, 55, u64);
define_bit_type!(B56, 56, u64);
define_bit_type!(B57, 57, u64);
define_bit_type!(B58, 58, u64);
define_bit_type!(B59, 59, u64);
define_bit_type!(B60, 60, u64);
define_bit_type!(B61, 61, u64);
define_bit_type!(B62, 62, u64);
define_bit_type!(B63, 63, u64);
define_bit_type!(B64, 64, u64);
