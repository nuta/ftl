use riscv::{instructions::wfi, sbi};

mod backtrace;
mod boot;
mod page_table;
mod switch;
mod thread;

pub const PAGE_SIZE: usize = 4096;
use crate::address::{PAddr, VAddr};
pub use backtrace::backtrace;
pub use page_table::{
    Page4K, PageTable, PageTableL0, PageTableL1, PageTableL2,
};
pub use thread::Context;

pub fn read_cpuvar_addr() -> usize {
    let tp: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) tp);
    }

    tp
}

pub fn write_cpuvar_addr(base: usize) {
    unsafe {
        core::arch::asm!("mv tp, {}", in(reg) base);
    }
}

pub fn owns_giant_lock() -> bool {
    true // FIXME:
}

pub const fn is_valid_vaddr(addr: usize) -> bool {
    // FIXME:
    0x80000000 <= addr && addr < 0x88000000
}

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    if paddr.as_usize() < 0x80000000 {
        return None;
    }

    // FIXME: use kernel-mapped region

    Some(VAddr::new(paddr.as_usize()))
}

pub fn vaddr2paddr(vaddr: VAddr) -> PAddr {
    debug_assert!(vaddr.as_usize() > 0x80000000);
    debug_assert!(vaddr.as_usize() < 0x88000000); // FIXME: use kernel-mapped region
    PAddr::new(vaddr.as_usize())
}

// pub fn read_cpu_cycles() -> usize {
//     rdcycle() as usize
// }

pub fn shutdown() {
    sbi::shutdown();
}

pub fn hang() -> ! {
    // TODO: remove this
    shutdown();

    loop {
        wfi();
    }
}

pub fn console_write(bytes: &[u8]) {
    for b in bytes {
        // Ignore errors. We can't do anything if something goes wrong
        // anyway.
        let _ = sbi::console_putchar(*b);
    }
}
