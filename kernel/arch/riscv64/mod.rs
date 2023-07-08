use riscv::{instructions::wfi, sbi};

mod boot;
mod page_table;
mod switch;
mod thread;

pub const PAGE_SIZE: usize = 4096;
pub use page_table::PageTable;
pub use thread::Context;

pub fn read_cpulocal_base() -> usize {
    let tp: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) tp);
    }

    debug_assert!(tp != 0);
    tp
}

pub fn write_cpulocal_base(base: usize) {
    unsafe {
        core::arch::asm!("mv tp, {}", in(reg) base);
    }
}

pub fn owns_giant_lock() -> bool {
    true // FIXME:
}

pub const fn is_valid_vaddr(addr: usize) -> bool {
    // FIXME:
    0x80000000 <= addr
}

// pub fn read_cpu_cycles() -> usize {
//     rdcycle() as usize
// }

pub fn shutdown() {
    sbi::shutdown();
}

pub fn hang() -> ! {
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
