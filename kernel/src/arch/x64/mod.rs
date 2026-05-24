mod boot;
mod console;
mod cpuvar;
mod idle;
mod idt;
mod ioport;
mod multiboot;
mod pic;
mod pvh;
mod syscall;
mod thread;
mod vmspace;

pub const NUM_CPUS_MAX: usize = 8;

pub use console::console_write;
pub use cpuvar::CpuVar;
pub use cpuvar::get_cpuvar;
pub use cpuvar::set_cpuvar;
pub use idle::idle;
pub use thread::Thread;
pub use vmspace::DIRECT_MAP_END;
pub use vmspace::MIN_PAGE_SIZE;
pub use vmspace::PageAttrs;
pub use vmspace::VmSpace;
pub use vmspace::paddr2vaddr;
pub use vmspace::vaddr2paddr;
