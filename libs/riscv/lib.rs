#![no_std]
#![cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]

pub mod instructions;
pub mod registers;
pub mod sbi;
