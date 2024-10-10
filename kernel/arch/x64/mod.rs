#![allow(unused)]
use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::interrupt::Irq;

use crate::cpuvar::CpuId;
use crate::interrupt::Interrupt;
use crate::refcount::SharedRef;

mod cpuvar;
mod idle;
mod serial;
mod switch;
mod thread;
mod vmspace;

pub use cpuvar::get_cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
pub use idle::idle;
pub use switch::return_to_user;
pub use thread::Thread;
pub use vmspace::VmSpace;
pub use vmspace::USERSPACE_END;
pub use vmspace::USERSPACE_START;

const KERNEL_BASE: usize = 0xffff_8000_0000_0000;

pub fn halt() -> ! {
    todo!()
}

pub fn paddr2vaddr(paddr: PAddr) -> Result<VAddr, FtlError> {
    Ok(VAddr::new(paddr.as_usize() + KERNEL_BASE))
}

pub fn vaddr2paddr(vaddr: VAddr) -> Result<PAddr, FtlError> {
    Ok(PAddr::new(vaddr.as_usize() - KERNEL_BASE))
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

pub fn early_init(cpu_id: CpuId) {
    const CR4_FSGSBASE: u64 = 1 << 16;
    unsafe {
        let mut cr4: u64;
        core::arch::asm!("mov rax, cr4", out("rax") cr4);
        cr4 |= CR4_FSGSBASE;
        core::arch::asm!("mov cr4, rax", in("rax") cr4);
    }
}

pub fn init(cpu_id: CpuId, device_tree: Option<&crate::device_tree::DeviceTree>) {}

pub const PAGE_SIZE: usize = 4096;
pub const NUM_CPUS_MAX: usize = 8;
