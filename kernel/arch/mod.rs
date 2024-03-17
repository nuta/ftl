#[cfg(all(target_family = "ftl", target_arch = "riscv64"))]
mod riscv64;

#[cfg(not(target_family = "ftl"))]
mod host;

#[cfg(not(target_family = "ftl"))]
pub use host::*;
#[cfg(all(target_family = "ftl", target_arch = "riscv64"))]
pub use riscv64::*;
