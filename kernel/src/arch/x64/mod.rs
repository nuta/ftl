mod boot;
mod vmspace;
mod console;
mod ioport;

pub const KERNEL_BASE: u64 = 0xffff_8000_0000_0000;

pub use console::console_write;
