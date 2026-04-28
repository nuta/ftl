use core::mem::MaybeUninit;

use ftl_arrayvec::ArrayVec;
use ftl_types::error::ErrorCode;
use ftl_types::time::Monotonic;
use ftl_types::vmspace::PageAttrs;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::boot::BootInfo;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const KERNEL_BASE: usize = 0xffff_8000_0000_0000;

static mut CPU_VAR: MaybeUninit<crate::cpuvar::CpuVar> = MaybeUninit::uninit();

pub fn console_write(_bytes: &[u8]) {}

pub fn main() -> ! {
    crate::boot::boot(&BootInfo {
        free_rams: ArrayVec::new(),
        initfs: &[],
    });
}

pub fn idle() -> ! {
    todo!()
}

pub fn halt() -> ! {
    todo!()
}

pub fn interrupt_acquire(_irq: u8) -> Result<(), ErrorCode> {
    Ok(())
}

pub fn interrupt_acknowledge(_irq: u8) {}

pub extern "C" fn direct_syscall_handler(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    n: usize,
) -> usize {
    crate::syscall::syscall_handler(a0, a1, a2, a3, a4, n)
}

pub fn read_timer() -> Monotonic {
    Monotonic::from_nanos(0)
}

pub fn set_timer(_deadline: Monotonic) {}

pub fn paddr2vaddr(paddr: PAddr) -> VAddr {
    VAddr::new(paddr.as_usize())
}

pub struct CpuVar;

impl CpuVar {
    pub fn new(_cpu_id: usize) -> Self {
        Self
    }
}

pub fn get_cpuvar() -> &'static crate::cpuvar::CpuVar {
    unsafe { &*(&raw const CPU_VAR).cast::<crate::cpuvar::CpuVar>() }
}

pub fn set_cpuvar(_cpu_id: usize, value: crate::cpuvar::CpuVar) {
    unsafe {
        (&raw mut CPU_VAR).write(MaybeUninit::new(value));
    }
}

#[derive(Default)]
#[repr(C)]
pub struct Thread {
    retval: usize,
}

impl Thread {
    pub fn new_kernel(_entry: usize, _sp: usize, _start_info: usize) -> Self {
        Self::default()
    }

    pub fn new_user(_entry: usize, _sp: usize, _start_info: usize) -> Self {
        Self::default()
    }

    pub fn new_idle() -> Self {
        Self::default()
    }

    pub fn set_syscall_result(&mut self, retval: usize) {
        self.retval = retval;
    }
}

pub fn thread_switch(_thread: *const Thread) -> ! {
    todo!()
}

pub struct VmSpace;

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self)
    }

    pub fn switch(&self) {}

    pub fn map(
        &self,
        _uaddr: usize,
        _paddr: PAddr,
        _len: usize,
        _attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }
}
