//! This crate provides `#[bitfields]`, an attribute macro to define bitfield structs
//! like MMIO registers, control registers in x86-64, and CSR registers in RISC-V.
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
//! #[bitfields(u32)]
//! #[derive(Copy, Clone)]
//! struct Stvec {
//!     mode: TrapMode,
//!     addr: B30,
//! }
//!
//! #[bitfields(bits = 2)]
//! #[derive(PartialEq)]
//! enum TrapMode {
//!     Direct = 0b00,
//!     Vectored = 0b01,
//!     // Enum must be exhaustive:
//!     Reserved1 = 0b10,
//!     Reserved2 = 0b11,
//! }
//!
//! assert_eq!(size_of::<Stvec>(), size_of::<u32>());
//! let mut stvec = Stvec::zeroed();
//! assert_eq!(stvec.mode(), TrapMode::Direct);
//! assert_eq!(stvec.addr(), 0);
//!
//! stvec.set_mode(TrapMode::Vectored);
//! stvec.set_addr(0x1234567);
//! assert_eq!(stvec.mode(), TrapMode::Vectored);
//! assert_eq!(stvec.addr(), 0x1234567);
//! ```
//!
//! # Defining a bitfield struct
//!
//! To define a bitfield struct, use `#[bitfields]` attribute macro:
//!
//! ```
//! use bitfields::{bitfields, B1, B2, B8, B21};
//!
//! #[bitfields(u32)]      // Automatically derives Debug.
//! #[derive(Copy, Clone)] // `#[derive]` must come after `#[bitfields]`.
//! struct MyStruct {
//!    a: B2,        // 2-bits
//!    b: bool,      // 1-bit but as bool
//!    #[bitfield(hidden)] // Don't expose this field.
//!    padding: B21, // 21-bits
//!    c: B8,        // 8-bits
//! }
//! ```
//!
//! ## Automatically generated methods
//!
//! `#[bitfields]` generates the following methods:
//!
//! ```ignore
//! Struct::zeroed() -> Struct        // Returns a zeroed struct.
//! Struct::from_raw(uXX) -> Struct   // Returns a struct from a raw value.
//! Struct::into_raw(self) -> uXX     // Returns a raw value from a struct.
//! Struct::FIELD_offset() -> usize   // Returns the offset of `FIELD`.
//! Struct::FIELD_width() -> usize    // Returns the bits of `FIELD`.
//! Struct::FIELD_range() -> RangeInclusive<usize> // Returns the bit range of `FIELD`.
//!
//! Struct::FIELD(&self) -> TYPE       // Returns the value of `FIELD`.
//! Struct::set_FIELD(&mut self, TYPE) // Sets the value of `FIELD`.
//! ```
//!
//! where `Struct` is the name of the struct, `FIELD` is the name of the field,
//! `uXX` is the underlying container type (e.g., `u8`, `u16`, `u32`, `u64`).
//!
//! `TYPE` is the type of the field. If the field is bit types (e.g., `B1`, `B2`, `B3`),
//! the type will be the smallest unsigned integer type that can hold the bits of the field:
//! `u8` for `B1-B8`, `u16` for `B9-B16`, `u32` for `B17-B32`, and `u64` for `B33-B64`.
//!
//! If the field is an enum, `TYPE` will be the enum type itself.
//!
//! ### What if I give an invalid bit pattern to `into_raw`?
//!
//! You may wonder why `into_raw` doesn't return `Option<T>`. It always succeeds, and won't
//! panic. This comes from the design principle of this crate: be always aware of invalid
//! bit patterns.
//!
//! Checking the validity of bit patterns could result in non-negligible overhead,
//! so we don't do it by default. Instead, it enforces you to handle invalid patterns explicitly.
//!
//! ## `#[bitfields]` attribute macro
//!
//! `#[bitfields]` attribute macro takes a single argument, which is the type of the
//! underlying container. The container type must be one of `u8`, `u16`, `u32`, `u64`.
//! It replaces the original struct with a new struct that has the same name, but
//! it has a single private field of the given container type.
//!
//! ### Warning: it must be the first attribute of the struct!
//!
//! It must be the first attribute of the struct: as describe above, it replaces fields
//! of the struct. If you put other attributes before `#[bitfields]`, they will be
//! applied to old fields, not new fields!
//!
//! ```ignore
//! #[bitfields(u32)]       // Should be the first attribute!
//! #[derive(Copy, Clone)]  // This will be applied to the new struct.
//! struct MyStruct {
//!     ...
//! }
//! ```
//!
//! ## Automatically derived traits
//!
//! `#[bitfields]` automatically derives `Debug` trait.
//!
//! ## Ordering of fields
//!
//! While `repr(Rust)` (default representation) reorders the fields to minimize the
//! size of the struct, `#[bitfields]` preserves the order of the fields. That is,
//! `a` will be the least significant bits, and `c` will be the most significant bits.
//!
//! ## `#[bitfield]` attribute
//!
//! `#[bitfield]` attribute can be used for each field. Please note that it's named
//! `bitfield` (singular), not `bitfields` (plural). Following attributes are available:
//!
//! - `#[bitfield(hidden)]`: Make the field hidden. Accessors for the field will not
//!   be generated.
//! - `#[bitfield(readonly)]`: Make the field read-only. Only a getter will be generated.
//! - `#[bitfield(writeonly)]`: Make the field write-only. Only a setter will be generated.
//!
//! ## Padding
//!
//! The sum of filed sizes must be equal to the size of the container type. You need
//! to add padding fields to make it so. In order not to expose accessors for padding
//! fields, add `#[bitfield(hidden)]` attribute to the field.
//!
//! # Defining an enum field
//!
//! You can define and use an enum as a field of a bitfield struct. The enum must be
//! annotated with `#[bitfields(bits = N)]` attribute, where `N` is the number of bits
//! the enum occupies:
//!
//! ```
//! use bitfields::{bitfields, B30};
//! use std::mem::size_of;
//!
//! #[bitfields(u32)]
//! #[derive(Copy, Clone)]
//! struct Stvec {
//!     mode: TrapMode,
//!     addr: B30,
//! }
//!
//! #[bitfields(bits = 2)]
//! #[derive(PartialEq)]
//! enum TrapMode {
//!     Direct = 0b00,
//!     Vectored = 0b01,
//!     // Enum must be exhaustive:
//!     Reserved1 = 0b10,
//!     Reserved2 = 0b11,
//! }
//! ```
//!
//! ## Non-exhaustive enums
//!
//! We say an enum is *non-exhaustive* if it has a variant that is not explicitly defined. For example,
//! in 2-bits-wide enum, it has 4 possible values: `0b00`, `0b01`, `0b10`, and `0b11`. If the enum
//! has less than 4 variants, it is non-exhaustive.
//!
//! If the enum is non-exhaustive, `#[bitfields]` will error out
//!
//! ## Automatically derived traits
//!
//! `#[bitfields]` automatically derives `Debug` trait.
#![no_std]
pub use bitfields_derive::bitfields;

/// Represents a field in a bitfield struct.
pub trait BitField {
    /// The number of bits the field occupies.
    const BITS: usize;
    /// The type of the value to access the field.
    type AccessorValueType;

    /// Creates a value of the field from the given raw value.
    fn from_u64(value: u64) -> Self::AccessorValueType;
    /// Converts the value of the field to the raw value.
    fn into_u64(value: Self::AccessorValueType) -> u64;
}

impl BitField for bool {
    const BITS: usize = 1;
    type AccessorValueType = bool;

    fn from_u64(value: u64) -> bool {
        value != 0
    }

    fn into_u64(value: bool) -> u64 {
        if value {
            1
        } else {
            0
        }
    }
}

macro_rules! define_bit_type {
    ($name:ident, $bits:literal, $value_ty:ty) => {
        #[doc = concat!($bits, "-bits-wide field.")]
        pub struct $name;

        impl BitField for $name {
            const BITS: usize = $bits;
            type AccessorValueType = $value_ty;

            fn from_u64(value: u64) -> $value_ty {
                debug_assert!((value as u64) <= ((1u128 << $bits) - 1) as u64);

                value as $value_ty
            }

            fn into_u64(value: $value_ty) -> u64 {
                value as u64
            }
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
