use core::ops::Range;
use core::slice;

use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch;

pub struct FreeRam {
    pub addr: PAddr,
    pub size: usize,
}

#[allow(unused)]
pub struct Module {
    pub start: PAddr,
    pub end: PAddr,
}

impl Module {
    pub fn as_bytes(&self) -> &[u8] {
        let start: *const u8 = arch::paddr2vaddr(self.start).as_ptr();
        let end: *const u8 = arch::paddr2vaddr(self.end).as_ptr();
        let len = (end as usize).saturating_sub(start as usize);
        unsafe { slice::from_raw_parts(start, len) }
    }
}

pub const NUM_MODULES_MAX: usize = 8;

pub struct BootInfo {
    pub cmdline: &'static [u8],
    #[allow(unused)]
    pub modules: ArrayVec<Module, NUM_MODULES_MAX>,
    pub free_rams: ArrayVec<FreeRam, 8>,
    pub reserved_regions: ArrayVec<Range<PAddr>, { NUM_MODULES_MAX + 2 }>,
}

pub fn boot(mut bootinfo: BootInfo) -> ! {
    crate::memory::init(&mut bootinfo);
    crate::cpuvar::init(0);
    crate::driver::init(&bootinfo.cmdline);
    crate::net::init();
    crate::loader::load(&bootinfo);
    trace!("kernel is ready");
    crate::scheduler::return_to_user();
}
