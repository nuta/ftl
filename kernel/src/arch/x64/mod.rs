mod boot;
mod bootinfo;
mod console;
mod cpuvar;
mod idle;
mod idt;
mod io_apic;
mod ioport;
mod local_apic;
mod mp_table;
mod multiboot;
mod pci;
mod pic;
mod pvh;
mod syscall;
mod thread;
mod vmspace;

pub use console::console_write;
pub use cpuvar::CpuVar;
pub use cpuvar::get_cpuvar;
pub use cpuvar::set_cpuvar;
pub use idle::idle;
pub use ioport::sys_x64_iopl;
pub use pci::sys_pci_get_bar;
pub use pci::sys_pci_get_interrupt_line;
pub use pci::sys_pci_lookup;
pub use pci::sys_pci_set_busmaster;
pub use syscall::direct_syscall_handler;
pub use thread::Thread;
pub use thread::thread_switch;
pub use vmspace::MIN_PAGE_SIZE;
pub use vmspace::paddr2vaddr;

pub use io_apic::interrupt_acquire;
pub use io_apic::interrupt_acknowledge;

pub(super) const NUM_CPUS_MAX: usize = 16;
