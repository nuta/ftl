use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::boot::BootInfo;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const DIRECT_MAP_END: PAddr = PAddr::new(usize::MAX);

pub fn console_write(_bytes: &[u8]) {}

pub fn paddr2vaddr(_paddr: PAddr) -> VAddr {
    todo!()
}

pub fn vaddr2paddr(_vaddr: VAddr) -> PAddr {
    todo!()
}

pub fn main() -> ! {
    crate::boot::boot(BootInfo {
        free_rams: ArrayVec::new(),
        modules: ArrayVec::new(),
    });
}
