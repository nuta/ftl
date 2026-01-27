use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::cpuvar;
use crate::initfs::InitFs;
use crate::loader;
use crate::memory;
use crate::thread::return_to_user;

#[derive(Debug)]
pub struct FreeRam {
    pub base: PAddr,
    pub size: usize,
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

extern "C" fn thread_entry(arg: usize) -> ! {
    unsafe extern "C" {
        fn direct_syscall_handler(a0: usize);
    }

    loop {
        unsafe {
            direct_syscall_handler(arg);
        }
    }
}
