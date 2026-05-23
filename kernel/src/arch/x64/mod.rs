mod boot;
mod console;
mod ioport;
mod multiboot;
mod pvh;
mod vmspace;

pub const NUM_CPUS_MAX: usize = 8;

pub use console::console_write;
pub use vmspace::DIRECT_MAP_END;
pub use vmspace::MIN_PAGE_SIZE;
pub use vmspace::paddr2vaddr;
pub use vmspace::vaddr2paddr;
