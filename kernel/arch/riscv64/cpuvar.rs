use core::{arch::asm, ptr::{self, NonNull}};

use super::thread::Context;

pub struct CpuVar {
    pub(super) context: *mut Context,
}

impl CpuVar {
    pub const fn new() -> Self {
        Self { context: ptr::null_mut() }
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

pub fn set_cpuvar(cpuvar: *mut crate::cpuvar::CpuVar) {
    // Store the address of the current CPU's `CpuVar` to `tp`.
    unsafe {
        asm!("mv tp, {}", in(reg) cpuvar);
    }
}
