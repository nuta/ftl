use core::{arch::asm};

use super::thread::Context;

pub struct CpuVar {
    pub(super) context: Context,
}

impl CpuVar {
    pub const fn new() -> Self {
        Self { context: Context::default() }
    }
}

pub fn cpuvar() -> &'static crate::cpuvar::CpuVar {
    // Load the address of the current CPU's `CpuVar` from `tp`.
    let cpuvar: *const crate::cpuvar::CpuVar;
    unsafe {
        asm!("mv {}, tp", out(reg) cpuvar);
    }
    unsafe { &*cpuvar }
}

pub fn set_cpuvar(cpuvar: *const crate::cpuvar::CpuVar) {
    // Store the address of the current CPU's `CpuVar` to `tp`.
    unsafe {
        asm!("mv tp, {}", in(reg) cpuvar);
    }
}
