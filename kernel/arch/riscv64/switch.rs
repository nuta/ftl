use core::arch::asm;
use core::arch::global_asm;
use core::mem::offset_of;

use super::thread::Context;
use crate::scheduler::GLOBAL_SCHEDULER;
use crate::thread::ContinuationResult;

global_asm!(
    r#"
.text
.global do_idle, __wfi_point
do_idle:
    fence
    csrsi sstatus, 1 << 1
__wfi_point:
    wfi
    csrci sstatus, 1 << 1
    ret
"#
);

extern "C" {
    fn do_idle();
    pub static __wfi_point: u8;
}

fn idle() -> ! {
    loop {
        unsafe {
            do_idle();
        }
    }
}

pub fn return_to_user() -> ! {
    loop {
        let mut current_thread = super::cpuvar().current_thread.borrow_mut();

        // Preemptive scheduling: push the current thread back to the
        // runqueue if it's still runnable.
        let thread_to_enqueue = if current_thread.is_runnable() && !current_thread.is_idle_thread()
        {
            Some(current_thread.clone())
        } else {
            None
        };

        // Get the next thread to run. If the runqueue is empty, run the
        // idle thread.
        let next = match GLOBAL_SCHEDULER.schedule(thread_to_enqueue) {
            Some(next) => next,
            None => {
                drop(current_thread);
                idle();
            }
        };

        // Make the next thread the current thread.
        *current_thread = next;

        // Switch to the new thread's address space.
        if let Some(vmspace) = current_thread.arch().vmspace.as_ref() {
            vmspace.switch();
        }

        // Run the next thread.
        let context: *mut Context = &current_thread.arch().context as *const _ as *mut _;
        match current_thread.run_continuation() {
            ContinuationResult::Yield => {
                // The thread is blocked. Yield the CPU.
                continue;
            }
            ContinuationResult::ReturnToUser => {
                drop(current_thread);

                // No continuation to run that is, the thread is not blocked. Resume the user.
                restore_kernel_context(context);
            }
            ContinuationResult::ReturnToUserWith { ret } => {
                // FIXME:
                unsafe {
                    (*context).a0 = ret as usize;
                }
            }
        }
    }
}

fn restore_kernel_context(context: *const Context) -> ! {
    unsafe {
        asm!(
            r#"
                sd a0, {context_offset}(tp) // Update CpuVar.arch.context

                ld a1, {pc_offset}(a0)
                csrw sepc, a1
                ld a1, {sstatus_offset}(a0)
                csrw sstatus, a1

                // Restore general-purpose registers except tp.
                ld ra, {ra_offset}(a0)
                ld sp, {sp_offset}(a0)
                ld gp, {gp_offset}(a0)
                ld t0, {t0_offset}(a0)
                ld t1, {t1_offset}(a0)
                ld t2, {t2_offset}(a0)
                ld s0, {s0_offset}(a0)
                ld s1, {s1_offset}(a0)
                ld a1, {a1_offset}(a0)
                ld a2, {a2_offset}(a0)
                ld a3, {a3_offset}(a0)
                ld a4, {a4_offset}(a0)
                ld a5, {a5_offset}(a0)
                ld a6, {a6_offset}(a0)
                ld a7, {a7_offset}(a0)
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
                ld t3, {t3_offset}(a0)
                ld t4, {t4_offset}(a0)
                ld t5, {t5_offset}(a0)
                ld t6, {t6_offset}(a0)

                ld a0, {a0_offset}(a0)
                sret
            "#,
            in ("a0") context as usize,
            context_offset = const offset_of!(crate::arch::CpuVar, context),
            pc_offset = const offset_of!(Context, pc),
            sstatus_offset = const offset_of!(Context, sstatus),
            ra_offset = const offset_of!(Context, ra),
            sp_offset = const offset_of!(Context, sp),
            gp_offset = const offset_of!(Context, gp),
            t0_offset = const offset_of!(Context, t0),
            t1_offset = const offset_of!(Context, t1),
            t2_offset = const offset_of!(Context, t2),
            s0_offset = const offset_of!(Context, s0),
            s1_offset = const offset_of!(Context, s1),
            a0_offset = const offset_of!(Context, a0),
            a1_offset = const offset_of!(Context, a1),
            a2_offset = const offset_of!(Context, a2),
            a3_offset = const offset_of!(Context, a3),
            a4_offset = const offset_of!(Context, a4),
            a5_offset = const offset_of!(Context, a5),
            a6_offset = const offset_of!(Context, a6),
            a7_offset = const offset_of!(Context, a7),
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
            t3_offset = const offset_of!(Context, t3),
            t4_offset = const offset_of!(Context, t4),
            t5_offset = const offset_of!(Context, t5),
            t6_offset = const offset_of!(Context, t6),
            options(noreturn)
        )
    }
}
