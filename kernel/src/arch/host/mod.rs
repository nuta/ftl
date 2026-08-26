use std::ops::Range;

use ftl_arrayvec::ArrayVec;
use ftl_types::error::ErrorCode;
use ftl_types::thread::RegsKind;
use ftl_types::thread::SyscallRegs;
use ftl_types::vmspace::PageAttrs;

use crate::address::PAddr;
use crate::address::UAddr;
use crate::address::USlice;
use crate::address::VAddr;
use crate::boot::BootInfo;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const USER_ADDR_END: usize = 0x8000_0000;
pub const DIRECT_MAP_END: PAddr = PAddr::new(usize::MAX);

pub fn idle() -> ! {
    todo!()
}

pub fn console_write(_bytes: &[u8]) {}

pub unsafe fn usercopy_read(src: UAddr, dst: *mut u8, len: usize) -> Result<(), ErrorCode> {
    unsafe { core::ptr::copy_nonoverlapping(src.as_usize() as *const u8, dst, len) }
    Ok(())
}

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
    pub fn new(
        _pc: usize,
        _sp: usize,
        _fault_pc: usize,
        _cookie: usize,
    ) -> Result<Self, ErrorCode> {
        todo!()
    }

    pub fn get_syscall_regs(&self) -> SyscallRegs {
        todo!()
    }

    pub fn set_syscall_retval(&mut self, _retval: usize) {
        todo!()
    }

    pub fn write_regs(&mut self, _kind: RegsKind, _regs: USlice) -> Result<(), ErrorCode> {
        todo!()
    }

    pub fn copy_regs(&mut self, _source: &Self, _kind: RegsKind) -> Result<(), ErrorCode> {
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
