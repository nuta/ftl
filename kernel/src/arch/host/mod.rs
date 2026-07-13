use std::ops::Range;

use ftl_api::error::ErrorCode;
use ftl_api::thread::ContextData;
use ftl_api::thread::ContextKind;
use ftl_api::vmspace::PageAttrs;
use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::address::UAddr;
use crate::address::VAddr;
use crate::boot::BootInfo;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const DIRECT_MAP_END: PAddr = PAddr::new(usize::MAX);

pub fn idle() -> ! {
    todo!()
}

pub fn console_write(_bytes: &[u8]) {}

pub fn paddr2vaddr(_paddr: PAddr) -> VAddr {
    todo!()
}

pub struct VmSpace {}

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        todo!()
    }

    pub fn switch(&self) {
        todo!()
    }

    pub fn map(
        &self,
        _uaddr: UAddr,
        _paddr: PAddr,
        _len: usize,
        _attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        todo!()
    }
}

pub struct Thread {}

impl Thread {
    pub fn new() -> Self {
        todo!()
    }

    pub fn read_context(&self, _kind: ContextKind, _regs: &mut ContextData) {
        todo!()
    }

    pub fn write_context(&mut self, _kind: ContextKind, _regs: &ContextData) {
        todo!()
    }

    pub fn enter(_thread: *const Thread) -> ! {
        todo!()
    }
}

pub struct CpuVar {}

impl CpuVar {
    pub fn new(_cpu_id: usize) -> Self {
        Self {}
    }
}

pub fn get_cpuvar() -> &'static crate::cpuvar::CpuVar {
    todo!()
}

pub fn set_cpuvar(_cpu_id: usize, _cpuvar: crate::cpuvar::CpuVar) {
    todo!()
}

pub fn semihosting_exit() -> ! {
    todo!()
}

pub fn get_kernel_reserved_range() -> Range<PAddr> {
    todo!()
}

#[unsafe(no_mangle)]
pub fn main() -> ! {
    crate::boot::boot(BootInfo {
        cmdline: b"",
        free_rams: ArrayVec::new(),
        modules: ArrayVec::new(),
    });
}
