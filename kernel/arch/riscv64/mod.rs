use core::arch::asm;

use ftl_types::address::PAddr;

use crate::vm::KVAddr;
use crate::vm::KVAddrArchExt;

mod context;
mod sbi;

pub use context::Context;

pub const PAGE_SIZE: usize = 4096;

pub fn init(cpu_id: usize) {}

pub fn idle() {
    unsafe {
        asm!("wfi");
    }
}

pub fn hang() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
    for byte in bytes {
        sbi::console_putchar(*byte);
    }
}

impl KVAddrArchExt for KVAddr {
    fn paddr(&self) -> PAddr {
        PAddr::from_nonzero(self.vaddr().as_nonzero())
    }
}
