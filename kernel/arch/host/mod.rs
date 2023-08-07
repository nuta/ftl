use crate::address::{PAddr, VAddr};

pub const PAGE_SIZE: usize = 4096;

pub struct Context {}
pub struct PageTable {}

pub fn read_cpuvar_addr() -> usize {
    unimplemented!()
}

pub fn write_cpuvar_addr(base: usize) {
    unimplemented!()
}

pub fn owns_giant_lock() -> bool {
    unimplemented!()
}

pub const fn is_valid_vaddr(addr: usize) -> bool {
    unimplemented!()
}

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    unimplemented!()
}

pub fn vaddr2paddr(vaddr: VAddr) -> PAddr {
    unimplemented!()
}

pub fn shutdown() {
    unimplemented!()
}

pub fn hang() -> ! {
    unimplemented!()
}

pub fn console_write(bytes: &[u8]) {
    unimplemented!()
}
