#[cfg(all(target_arch = "riscv64"))]
mod riscv64;

#[cfg(all(target_arch = "x86_64"))]
mod x64;

#[cfg(all(target_arch = "riscv64"))]
pub use riscv64::*;

#[cfg(all(target_arch = "x86_64"))]
pub use x64::*;
