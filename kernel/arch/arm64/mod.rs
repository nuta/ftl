use core::arch::asm;

use ftl_types::address::{PAddr, VAddr};

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

pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
}

pub fn init() {
}
