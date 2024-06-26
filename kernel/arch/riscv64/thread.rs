use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::offset_of;
use core::mem::size_of;

use ftl_utils::byte_size::ByteSize;

use super::cpuvar::CpuVar;
use crate::scheduler::GLOBAL_SCHEDULER;

const KERNEL_STACK_SIZE: ByteSize = ByteSize::from_kib(64);

/// The entry point of kernel threads.
#[no_mangle]
#[naked]
extern "C" fn kernel_entry() -> ! {
    unsafe {
        asm!(
            r#"
                mv ra, s1    // The desired entry point for the thread.
                mv a0, s2    // The argument for the thread.
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
                ld a0, {context_offset}(tp)
                sd ra, {ra_offset}(a0)
                sd sp, {sp_offset}(a0)
                sd fp, {fp_offset}(a0)
                sd s1, {s1_offset}(a0)
                sd s2, {s2_offset}(a0)
                sd s3, {s3_offset}(a0)
                sd s4, {s4_offset}(a0)
                sd s5, {s5_offset}(a0)
                sd s6, {s6_offset}(a0)
                sd s7, {s7_offset}(a0)
                sd s8, {s8_offset}(a0)
                sd s9, {s9_offset}(a0)
                sd s10, {s10_offset}(a0)
                sd s11, {s11_offset}(a0)
                j {switch_to_next}
            "#,
            context_offset = const offset_of!(CpuVar, context),
            ra_offset = const offset_of!(Context, ra),
            sp_offset = const offset_of!(Context, sp),
            fp_offset = const offset_of!(Context, fp),
            s1_offset = const offset_of!(Context, s1),
            s2_offset = const offset_of!(Context, s2),
            s3_offset = const offset_of!(Context, s3),
            s4_offset = const offset_of!(Context, s4),
            s5_offset = const offset_of!(Context, s5),
            s6_offset = const offset_of!(Context, s6),
            s7_offset = const offset_of!(Context, s7),
            s8_offset = const offset_of!(Context, s8),
            s9_offset = const offset_of!(Context, s9),
            s10_offset = const offset_of!(Context, s10),
            s11_offset = const offset_of!(Context, s11),
            switch_to_next = sym switch_to_next,
            options(noreturn)
        )
    }
}

/// Resumes a thread.
fn resume(next: *mut Context) -> ! {
    unsafe {
        asm!(
            r#"
                // Update CpuVar.arch.context
                mv a0, {next_context}
                sd a0, {context_offset}(tp)

                ld ra, {ra_offset}(a0)
                ld sp, {sp_offset}(a0)
                ld fp, {fp_offset}(a0)
                ld s1, {s1_offset}(a0)
                ld s2, {s2_offset}(a0)
                ld s3, {s3_offset}(a0)
                ld s4, {s4_offset}(a0)
                ld s5, {s5_offset}(a0)
                ld s6, {s6_offset}(a0)
                ld s7, {s7_offset}(a0)
                ld s8, {s8_offset}(a0)
                ld s9, {s9_offset}(a0)
                ld s10, {s10_offset}(a0)
                ld s11, {s11_offset}(a0)
                ret
            "#,
            next_context = in (reg) next as usize,
            context_offset = const offset_of!(CpuVar, context),
            ra_offset = const offset_of!(Context, ra),
            sp_offset = const offset_of!(Context, sp),
            fp_offset = const offset_of!(Context, fp),
            s1_offset = const offset_of!(Context, s1),
            s2_offset = const offset_of!(Context, s2),
            s3_offset = const offset_of!(Context, s3),
            s4_offset = const offset_of!(Context, s4),
            s5_offset = const offset_of!(Context, s5),
            s6_offset = const offset_of!(Context, s6),
            s7_offset = const offset_of!(Context, s7),
            s8_offset = const offset_of!(Context, s8),
            s9_offset = const offset_of!(Context, s9),
            s10_offset = const offset_of!(Context, s10),
            s11_offset = const offset_of!(Context, s11),
            options(noreturn)
        )
    }
}

/// Context of a thread.
///
/// Only callee-saved registers are stored because caller-saved registers are
/// already saved on the stack.
#[derive(Debug, Default)]
#[repr(C)]
pub struct Context {
    ra: usize,
    sp: usize,
    fp: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}

// TODO: static assert to ensure usize == u64

pub struct Thread {
    pub(super) context: Context,
    /// The ownership of the stack area. To be freeed when the thread is destroyed.
    #[allow(dead_code)]
    stack: Option<Vec<u128>>,
}

impl Thread {
    pub fn new_idle() -> Thread {
        Thread {
            stack: None,
            context: Default::default(),
        }
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Thread {
        let stack_size = 64 * 1024;

        // Use Vec<u128> to ensure 16-byte alignment as specified in the RISC-V calling convention:
        //
        // > In the standard RISC-V calling convention, the stack grows downward and the stack pointer is
        // > always kept 16-byte aligned.
        //
        // TODO: Avoid initializing the stack with zeros.
        let stack: Vec<u128> = vec![0; KERNEL_STACK_SIZE.in_bytes() / size_of::<u128>()];

        let sp = (stack.as_ptr() as usize) + stack_size;
        Thread {
            stack: Some(stack),
            context: Context {
                ra: kernel_entry as usize,
                sp,
                // Zeroing fp is important so that backtrace stops at kernel_entry.
                fp: 0,
                s1: pc,
                s2: arg,
                ..Default::default()
            },
        }
    }

    pub fn resume(&self) -> ! {
        resume(&self.context as *const _ as *mut _);
    }
}
