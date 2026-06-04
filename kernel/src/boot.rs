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
    pub cmdline: &'static [u8],
    #[allow(unused)]
    pub modules: ArrayVec<Module, NUM_MODULES_MAX>,
    pub free_rams: ArrayVec<FreeRam, 8>,
}

pub fn boot(bootinfo: BootInfo) -> ! {
    crate::memory::init(&bootinfo);
    crate::cpuvar::init(0);
    crate::loader::init(&bootinfo);
    crate::arch::semihosting_exit();
    crate::scheduler::return_to_user();
}
