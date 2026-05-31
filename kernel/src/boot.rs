use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;

pub struct FreeRam {
    pub addr: PAddr,
    pub size: usize,
}

#[allow(unused)]
pub struct Module {
    pub start: PAddr,
    pub end: PAddr,
}

pub const NUM_MODULES_MAX: usize = 8;

pub struct BootInfo {
    #[allow(unused)]
    pub modules: ArrayVec<Module, NUM_MODULES_MAX>,
    pub free_rams: ArrayVec<FreeRam, 8>,
}

pub fn boot(bootinfo: BootInfo) -> ! {
    crate::memory::init(&bootinfo);
    crate::cpuvar::init(0);
    crate::loader::init();
    crate::thread::return_to_user();
}
