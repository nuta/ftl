use core::arch::asm;
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::arch::x64::boot::NUM_GDT_ENTRIES;
use crate::arch::x64::boot::Tss;

const MAGIC: u64 = 0xc12c_12c1_2c12_c12c;

/// CPU-local variables.
///
/// The kernel stack continues after the CPU variable.
#[repr(C)]
pub(super) struct CpuVar {
    pub(super) common: crate::cpuvar::CpuVar,
    pub(super) gdt: RefCell<[u64; NUM_GDT_ENTRIES]>,
    pub(super) tss: RefCell<Tss>,
    magic: u64,
}

pub fn get_cpuvar() -> &'static CpuVar {
    let cpuvar = unsafe {
        let gsbase: u64;
        asm!("rdgsbase {}", out(reg) gsbase);
        debug_assert!(gsbase != 0);
        &*(gsbase as *mut CpuVar)
    };

    debug_assert_eq!(cpuvar.magic, MAGIC);
    cpuvar
}

const NUM_CPUS_MAX: usize = 16;
static mut CPU_VARS: [MaybeUninit<CpuVar>; NUM_CPUS_MAX] =
    [const { MaybeUninit::uninit() }; NUM_CPUS_MAX];

pub fn init(cpu_id: usize, gdt: [u64; NUM_GDT_ENTRIES], tss: Tss) {
    assert!(cpu_id < NUM_CPUS_MAX);
    unsafe {
        let cpu_var = &mut CPU_VARS[cpu_id];
        cpu_var.write(CpuVar {
            // TODO: Do not initialize in arch.
            common: crate::cpuvar::CpuVar {
                current_thread: UnsafeCell::new(core::ptr::null()),
            },
            gdt: RefCell::new(gdt),
            tss: RefCell::new(tss),
            magic: MAGIC,
        });

        asm!("wrgsbase rax", in("rax") cpu_var.as_mut_ptr());
    }
}
