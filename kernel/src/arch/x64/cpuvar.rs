use core::arch::asm;
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::ptr;

use ftl_utils::static_assert;

use crate::arch::x64::boot::KERNEL_STACK_SIZE;
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
    /// The magic number to verify that the CPU-local variables are initialized
    /// and valid. This is placed at the end intentionally to detect stack
    /// corruption.
    magic: u64,
}

const SP_BOTTOM_MASK: u64 = !(KERNEL_STACK_SIZE as u64 - 1);
static_assert!(KERNEL_STACK_SIZE.is_power_of_two());

fn get_cpuvar_ptr() -> *mut CpuVar {
    let rsp: u64;
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp);
    }

    let stack_bottom = rsp & SP_BOTTOM_MASK;
    stack_bottom as *mut CpuVar
}

pub fn get_cpuvar() -> &'static CpuVar {
    let cpuvar_ptr = get_cpuvar_ptr();

    // SAFETY: This assumes rsp points to a valid kernel stack.
    let cpuvar = unsafe { &*cpuvar_ptr };

    debug_assert_eq!(cpuvar.magic, MAGIC);
    cpuvar
}

pub fn init(gdt: [u64; NUM_GDT_ENTRIES], tss: Tss) {
    unsafe {
        let cpuvar_ptr = get_cpuvar_ptr();

        asm!("wrgsbase rax", in("rax") cpuvar_ptr);

        ptr::write(
            cpuvar_ptr,
            CpuVar {
                // TODO: Do not initialize in arch.
                common: crate::cpuvar::CpuVar {
                    current_thread: UnsafeCell::new(core::ptr::null()),
                },
                gdt: RefCell::new(gdt),
                tss: RefCell::new(tss),
                magic: MAGIC,
            },
        );
    }
}
