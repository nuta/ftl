use alloc::borrow::Cow;
use core::ops::Range;
use core::slice;

use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch;
use crate::scheduler::Resched;

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
    #[allow(unused)]
    pub fn as_bytes(&self) -> &[u8] {
        let start: *const u8 = arch::paddr2vaddr(self.start).as_ptr();
        let end: *const u8 = arch::paddr2vaddr(self.end).as_ptr();
        let len = (end as usize).saturating_sub(start as usize);
        unsafe { slice::from_raw_parts(start, len) }
    }
}

pub const NUM_MODULES_MAX: usize = 8;

pub struct BootInfo {
    pub cmdline: Cow<'static, [u8]>,
    #[allow(unused)]
    pub modules: ArrayVec<Module, NUM_MODULES_MAX>,
    pub free_rams: ArrayVec<FreeRam, 8>,
    pub reserved_regions: ArrayVec<Range<PAddr>, { NUM_MODULES_MAX + 2 }>,
}

fn override_bootinfo(bootinfo: &mut BootInfo) {
    if let Some(append) = option_env!("APPEND_CMDLINE") {
        if !append.is_empty() {
            let cmdline = bootinfo.cmdline.to_mut();
            cmdline.push(b' ');
            cmdline.extend_from_slice(append.as_bytes());
        }
    }
}

pub fn boot(mut bootinfo: BootInfo) -> ! {
    crate::memory::init(&mut bootinfo);
    override_bootinfo(&mut bootinfo);
    crate::cpuvar::init(0);
    crate::driver::init(&bootinfo.cmdline);
    crate::loader::load(&bootinfo);

    // crate::loader copies the modules into VMOs. We can reuse the regions as
    // free memory.
    for module in bootinfo.modules.drain() {
        crate::memory::PAGE_ALLOCATOR.add_region(module.start, module.end);
    }

    trace!("kernel is ready");
    crate::scheduler::return_to_user(Resched::Init);
}
