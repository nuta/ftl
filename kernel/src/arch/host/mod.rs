use core::ops::BitOr;
use std::ops::Range;

use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::address::UAddr;
use crate::address::VAddr;
use crate::boot::BootInfo;
use crate::error::ErrorCode;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const DIRECT_MAP_END: PAddr = PAddr::new(usize::MAX);

pub fn idle() -> ! {
    todo!()
}

pub fn console_write(_bytes: &[u8]) {}

pub fn paddr2vaddr(_paddr: PAddr) -> VAddr {
    todo!()
}

#[derive(Clone, Copy)]
pub struct PageAttrs(usize);

impl PageAttrs {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXEC: Self = Self(1 << 2);
}

impl BitOr<Self> for PageAttrs {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
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
    pub fn new(_entry: UAddr, _sp: UAddr) -> Self {
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
