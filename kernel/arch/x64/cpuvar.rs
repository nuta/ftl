use core::arch::asm;
use core::cell::RefCell;

use super::gdt::Gdt;
use super::idt::Idt;
use super::thread::Context;
use super::tss::Tss;
use crate::ref_counted::SharedRef;
use crate::thread::Thread;

pub struct CpuVar {
    pub(super) context: *mut Context,
    pub(super) gdt: RefCell<Gdt>,
    pub(super) idt: Idt,
    pub(super) tss: Tss,
}

impl CpuVar {
    pub fn new(idle_thread: &SharedRef<Thread>) -> Self {
        Self {
            context: &idle_thread.arch().context as *const _ as *mut _,
            gdt: RefCell::new(Gdt::new()),
            idt: Idt::new(),
            tss: Tss::new(),
        }
    }
}

pub fn cpuvar() -> &'static crate::cpuvar::CpuVar {
    // Load the address of the current CPU's `CpuVar` from `tp`.
    let cpuvar: *const crate::cpuvar::CpuVar;
    unsafe {
        asm!("rdgsbase {}", out(reg) cpuvar);
    }
    unsafe { &*cpuvar }
}

pub fn set_cpuvar(cpuvar: *mut crate::cpuvar::CpuVar) {
    // Store the address of the current CPU's `CpuVar` to `tp`.
    unsafe {
        asm!("wrgsbase {}", in(reg) cpuvar);
    }
}
