use core::{
    hint,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use riscv::{instructions::wfi, sbi};

mod backtrace;
mod boot;
mod page_table;
mod switch;
mod thread;
mod trap;

pub const PAGE_SIZE: usize = 4096;
use crate::{
    address::{PAddr, VAddr},
    arch::riscv64::page_table::KERNEL_MAPPED_SIZE,
};
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

static GIANT_LOCK: AtomicBool = AtomicBool::new(false);

pub fn giant_lock() {
    while GIANT_LOCK
        .compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        )
        .is_err()
    {
        hint::spin_loop();
    }
}

pub fn giant_unlock() {
    GIANT_LOCK.store(false, Ordering::Release);
}

pub fn owns_giant_lock() -> bool {
    // TODO: Check owner CPU ID
    GIANT_LOCK.load(Ordering::Relaxed)
}

pub const fn is_valid_vaddr(addr: usize) -> bool {
    addr <= 0x80000000 && addr < 0x80000000 + KERNEL_MAPPED_SIZE
}

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    if is_valid_vaddr(paddr.as_usize()) {
        return None;
    }

    Some(VAddr::new(paddr.as_usize()))
}

pub fn vaddr2paddr(vaddr: VAddr) -> PAddr {
    // While VAddr is guaranteed to be valid acessible address, it might contain
    // an invalid address. Just check it to be sure.
    debug_assert!(is_valid_vaddr(vaddr.as_usize()));

    PAddr::new(vaddr.as_usize())
}

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
