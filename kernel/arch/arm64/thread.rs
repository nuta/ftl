use core::arch::asm;
use core::mem::offset_of;

use super::cpuvar::CpuVar;
use crate::folio::Folio;
use crate::scheduler::GLOBAL_SCHEDULER;

/// The entry point of kernel threads.
#[no_mangle]
#[naked]
extern "C" fn kernel_entry() -> ! {
    unsafe {
        asm!(
            r#"
                mov lr, x19   // The desired entry point for the thread.
                mov x0, x20    // The argument for the thread.
                ret
            "#,
            options(noreturn)
        )
    }
}

fn switch_to_next() {
    GLOBAL_SCHEDULER.yield_cpu();
}

/// # Why `#[naked]`?
///
/// - To get the correct return address from ra. #[naked] prevents inlining.
/// - To eliminate the needless prologue.
#[naked]
pub extern "C" fn yield_cpu() {
    unsafe {
        asm!(
            r#"
                // ld a0, {context_offset}(tp)
                b {switch_to_next}
            "#,
            context_offset = const offset_of!(CpuVar, context),
            switch_to_next = sym switch_to_next,
            options(noreturn)
        )
    }
}

/// Resumes a thread.
fn resume(next: *mut Context) -> ! {
    todo!()
}

/// Context of a thread.
///
/// Only callee-saved registers are stored because caller-saved registers are
/// already saved on the stack.
#[derive(Debug, Default)]
#[repr(C)]
pub struct Context {
    x19: usize,
    x20: usize,
    x21: usize,
    x22: usize,
    x23: usize,
    x24: usize,
    x25: usize,
    x26: usize,
    x27: usize,
    x28: usize,
    fp: usize, // aka x29
    lr: usize, // aka x30
    sp: usize,
}

pub struct Thread {
    pub(super) context: Context,
    #[allow(dead_code)]
    stack_folio: Option<Folio>,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            context: Default::default(),
            stack_folio: None,
        }
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Thread {
        let stack_size = 64 * 1024;
        let stack_folio = Folio::alloc(stack_size).unwrap();

        let sp = stack_folio.vaddr().unwrap().as_usize() + stack_size;
        Thread {
            context: Context {
                sp,
                x19: pc,
                x20: arg,
                ..Default::default()
            },
            stack_folio: Some(stack_folio),
        }
    }

    pub fn resume(&self) -> ! {
        resume(&self.context as *const _ as *mut _);
    }
}
