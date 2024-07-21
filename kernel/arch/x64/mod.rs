use core::arch::asm;

mod backtrace;
mod cpuvar;
mod interrupt;
mod thread;
mod gdt;
mod idt;
mod tss;
mod lapic;

pub use backtrace::backtrace;
pub use cpuvar::cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::interrupt::Irq;
pub use thread::yield_cpu;
pub use thread::Thread;

use crate::device_tree::DeviceTree;
use crate::interrupt::Interrupt;
use crate::ref_counted::SharedRef;

pub const PAGE_SIZE: usize = 4096;
pub const NUM_CPUS_MAX: usize = 8;

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    // Identical mapping.
    Some(VAddr::from_nonzero(paddr.as_nonzero()))
}

pub fn vaddr2paddr(vaddr: VAddr) -> Option<PAddr> {
    // Identical mapping.
    Some(PAddr::from_nonzero(vaddr.as_nonzero()))
}

pub fn create_interrupt(_interrupt: &SharedRef<Interrupt>) -> Result<(), FtlError> {
    todo!()
}

pub fn ack_interrupt(_irq: Irq) -> Result<(), FtlError> {
    todo!()
}

pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli; hlt");
        }
    }
}

pub fn idle() -> ! {
    loop {
        unsafe {
            asm!("sti; hlt; cli");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
    for byte in bytes {
        unsafe {
            asm!("out dx, al",
                in("dx") 0x3f8,
                in("al") *byte);
        }
    }
}

pub fn init(_device_tree: Option<&DeviceTree>) {
    unsafe {
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);

        cr4 |= 1 << 16; // FSGSBASE
        asm!("mov cr4, {}", in(reg) cr4);
    }
}
