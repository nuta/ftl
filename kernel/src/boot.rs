use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::cpuvar;
use crate::initfs::InitFs;
use crate::loader;
use crate::memory;
use crate::thread::return_to_user;

pub struct FreeRam {
    pub start: PAddr,
    pub end: PAddr,
}

pub struct BootInfo {
    pub free_rams: ArrayVec<FreeRam, 8>,
    pub initfs: &'static [u8],
}

pub fn boot(bootinfo: &BootInfo) -> ! {
    memory::init(bootinfo);
    cpuvar::init();
    let initfs = InitFs::new(bootinfo.initfs);
    loader::load(&initfs);
    return_to_user();
}
