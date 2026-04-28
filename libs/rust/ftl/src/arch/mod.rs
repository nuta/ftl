#[cfg(target_os = "none")]
pub mod x64;

#[cfg(not(target_os = "none"))]
pub mod host;

#[cfg(not(target_os = "none"))]
pub use host::*;
#[cfg(target_os = "none")]
pub use x64::*;
