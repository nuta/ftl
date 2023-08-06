#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(not(target_os = "none"))]
mod host;
#[cfg(not(target_os = "none"))]
pub use host::*;
