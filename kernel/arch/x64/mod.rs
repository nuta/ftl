use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::interrupt::Irq;

use crate::cpuvar::CpuId;
use crate::interrupt::Interrupt;
use crate::refcount::SharedRef;

mod cpuvar;
mod serial;
mod thread;

pub use cpuvar::get_cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
pub use thread::Thread;

pub fn halt() -> ! {
    todo!()
}

pub fn paddr2vaddr(paddr: PAddr) -> Result<VAddr, FtlError> {
    todo!()
}

pub fn vaddr2paddr(vaddr: VAddr) -> Result<PAddr, FtlError> {
    todo!()
}

pub fn console_write(bytes: &[u8]) {
    for ch in bytes {
        serial::SERIAL0.print_char(*ch);
    }
}

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize),
{
    todo!()
}

pub fn return_to_user() -> ! {
    todo!()
}

pub unsafe extern "C" fn kernel_syscall_entry(
    _a0: isize,
    _a1: isize,
    _a2: isize,
    _a3: isize,
    _a4: isize,
    _a5: isize,
    _a6: isize,
) -> isize {
    todo!()
}

pub fn interrupt_create(interrupt: &SharedRef<Interrupt>) -> Result<(), FtlError> {
    todo!()
}

pub fn interrupt_ack(irq: Irq) -> Result<(), FtlError> {
    todo!()
}

pub fn init(cpu_id: CpuId, device_tree: Option<&crate::device_tree::DeviceTree>) {
    todo!()
}

pub struct VmSpace {}

impl VmSpace {
    pub fn new() -> Result<VmSpace, FtlError> {
        todo!()
    }

    pub fn map_fixed(&self, vaddr: VAddr, paddr: PAddr, len: usize) -> Result<(), FtlError> {
        todo!()
    }

    pub fn map_anywhere(&self, paddr: PAddr, len: usize) -> Result<VAddr, FtlError> {
        todo!()
    }

    pub fn switch(&self) {
        todo!()
    }
}

pub const PAGE_SIZE: usize = 4096;
pub const NUM_CPUS_MAX: usize = 8;
pub const USERSPACE_START: VAddr = VAddr::new(0);
pub const USERSPACE_END: VAddr = VAddr::new(0);
