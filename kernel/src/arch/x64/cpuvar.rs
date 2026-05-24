use core::arch::asm;
use core::mem::MaybeUninit;

use super::NUM_CPUS_MAX;

const MAGIC: u64 = 0xc12c_12c1_2c12_c12c;

static mut CPU_VARS: [MaybeUninit<crate::cpuvar::CpuVar>; NUM_CPUS_MAX] =
    [const { MaybeUninit::uninit() }; NUM_CPUS_MAX];

/// CPU-local variables.
#[repr(C)]
pub struct CpuVar {
    magic: u64,
    pub(super) kernel_rsp: u64,
}

impl CpuVar {
    pub fn new(cpu_id: usize) -> Self {
        let kernel_rsp = if cpu_id == 0 {
            super::boot::bsp_stack_top()
        } else {
            todo!()
        };

        Self {
            magic: MAGIC,
            kernel_rsp,
        }
    }
}

pub fn get_cpuvar() -> &'static crate::cpuvar::CpuVar {
    let cpuvar = unsafe {
        let gsbase: u64;
        asm!("rdgsbase {}", out(reg) gsbase);
        debug_assert!(gsbase != 0);
        &*(gsbase as *mut crate::cpuvar::CpuVar)
    };

    debug_assert_eq!(cpuvar.arch.magic, MAGIC);
    cpuvar
}

pub fn set_cpuvar(cpu_id: usize, value: crate::cpuvar::CpuVar) {
    assert!(cpu_id < NUM_CPUS_MAX);
    unsafe {
        let cpu_var = &mut CPU_VARS[cpu_id];
        cpu_var.write(value);
        asm!("wrgsbase rax", in("rax") cpu_var.as_mut_ptr());
    }
}
