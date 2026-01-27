use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch::paddr2vaddr;
use crate::cpuvar;
use crate::initfs::InitFs;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::{self};
use crate::scheduler::SCHEDULER;
use crate::thread::Thread;
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

    for file in initfs.iter() {
        println!(
            "file: {}: {:x}, {:x}, {:x}, {:x} ({} bytes)",
            file.name,
            file.data[0],
            file.data[1],
            file.data[2],
            file.data[3],
            file.data.len()
        );
    }

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
