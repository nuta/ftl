use core::{arch::asm, mem::size_of};
use core::mem::offset_of;

use super::cpuvar::CpuVar;

use alloc::vec;
use ftl_utils::byte_size::ByteSize;

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

extern "C" fn switch_to_next() {
    // Scheduler::switch_to_next(GLOBAL_SCHEDULER.lock());
    todo!()
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
                sd ra, {ra_offset}(tp)
                sd sp, {sp_offset}(tp)
                sd fp, {fp_offset}(tp)
                sd s1, {s1_offset}(tp)
                sd s2, {s2_offset}(tp)
                sd s3, {s3_offset}(tp)
                sd s4, {s4_offset}(tp)
                sd s5, {s5_offset}(tp)
                sd s6, {s6_offset}(tp)
                sd s7, {s7_offset}(tp)
                sd s8, {s8_offset}(tp)
                sd s9, {s9_offset}(tp)
                sd s10, {s10_offset}(tp)
                sd s11, {s11_offset}(tp)
                j {switch_to_next}
            "#,
            ra_offset = const offset_of!(CpuVar, context) + offset_of!(Context, ra),
            sp_offset = const offset_of!(CpuVar, context) + offset_of!(Context, sp),
            fp_offset = const offset_of!(CpuVar, context) + offset_of!(Context, fp),
            s1_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s1),
            s2_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s2),
            s3_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s3),
            s4_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s4),
            s5_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s5),
            s6_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s6),
            s7_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s7),
            s8_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s8),
            s9_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s9),
            s10_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s10),
            s11_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s11),
            switch_to_next = sym switch_to_next,
            options(noreturn)
        )
    }
}

/// Restores an in-kernel Fiber context from ssctrach.
// #[naked]
pub extern "C" fn restore_context() -> ! {
    unsafe {
        asm!(
            r#"
                ld ra, {ra_offset}(tp)
                ld sp, {sp_offset}(tp)
                ld fp, {fp_offset}(tp)
                ld s1, {s1_offset}(tp)
                ld s2, {s2_offset}(tp)
                ld s3, {s3_offset}(tp)
                ld s4, {s4_offset}(tp)
                ld s5, {s5_offset}(tp)
                ld s6, {s6_offset}(tp)
                ld s7, {s7_offset}(tp)
                ld s8, {s8_offset}(tp)
                ld s9, {s9_offset}(tp)
                ld s10, {s10_offset}(tp)
                ld s11, {s11_offset}(tp)
                ret
            "#,
            ra_offset = const offset_of!(CpuVar, context) + offset_of!(Context, ra),
            sp_offset = const offset_of!(CpuVar, context) + offset_of!(Context, sp),
            fp_offset = const offset_of!(CpuVar, context) + offset_of!(Context, fp),
            s1_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s1),
            s2_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s2),
            s3_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s3),
            s4_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s4),
            s5_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s5),
            s6_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s6),
            s7_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s7),
            s8_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s8),
            s9_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s9),
            s10_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s10),
            s11_offset = const offset_of!(CpuVar, context) + offset_of!(Context, s11),
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

impl Context {
    pub fn new_kernel(pc: usize, arg: usize) -> Self {
        let stack_size = 64 * 1024;
        // Use Vec<u128> to ensure 16-byte alignment as specified in the RISC-V calling convention:
        //
        // > In the standard RISC-V calling convention, the stack grows downward and the stack pointer is
        // > always kept 16-byte aligned.
        //
        // TODO: Avoid initializing the stack with zeros.
        let stack = vec![0u128; KERNEL_STACK_SIZE.in_bytes() / size_of::<u128>()];
        let sp_bottom = stack.as_ptr() as usize;
        let sp = sp_bottom + stack_size;
        Self {
            ra: kernel_entry as usize,
            sp,
            // Zeroing fp is important so that backtrace stops at kernel_entry.
            fp: 0,
            s1: pc,
            s2: arg,
            ..Default::default()
        }
    }
}

pub struct Thread {}
