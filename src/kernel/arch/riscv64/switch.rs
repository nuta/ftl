use core::{arch::asm, mem::offset_of};

use super::thread::{current_thread, current_thread_mut, Context};

// Should never return.
extern "C" fn trap_handler() -> ! {
    let scause: u64;
    unsafe {
        asm!("csrr {}, scause", out(reg) scause);
    }

    let sepc: u64;
    unsafe {
        asm!("csrr {}, sepc", out(reg) sepc);
    }

    current_thread_mut().context.pc += 4;
    println!(
        "trap_handler: scause={:x}, sepc={:x}, a0={:x}",
        scause,
        sepc,
        current_thread().context.a0
    );
    if current_thread().context.a0 == 0xdead {
        panic!("User program exited");
    }
    super::thread::Thread::switch_test();
}

// The interrupt/exception/system call handler entry point. `stvec` is set to
// this address.
//
// This function address must be aligned to 4 bytes not to accidentally set
// MODE field in stvec.
#[naked]
#[repr(align(4))]
pub unsafe extern "C" fn switch_to_kernel() -> ! {
    asm!(
        r#"
        csrrw tp, sscratch, tp
        sd ra, {ra_offset}(tp)
        sd sp, {sp_offset}(tp)
        sd gp, {gp_offset}(tp)
        sd t0, {t0_offset}(tp)
        sd t1, {t1_offset}(tp)
        sd t2, {t2_offset}(tp)
        sd t3, {t3_offset}(tp)
        sd t4, {t4_offset}(tp)
        sd t5, {t5_offset}(tp)
        sd t6, {t6_offset}(tp)
        sd a0, {a0_offset}(tp)
        sd a1, {a1_offset}(tp)
        sd a2, {a2_offset}(tp)
        sd a3, {a3_offset}(tp)
        sd a4, {a4_offset}(tp)
        sd a5, {a5_offset}(tp)
        sd a6, {a6_offset}(tp)
        sd a7, {a7_offset}(tp)
        sd s0, {s0_offset}(tp)
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

        // set the kernel stack
        // ld sp, kernel_sp_offset(tp) FIXME:

        // user pc
        csrr a0, sepc
        sd a0, {pc_offset}(tp)

        // sstatus
        csrr a0, sstatus
        sd a0, {sstatus_offset}(tp)

        // user tp
        csrr a0, sscratch
        sd a0, {tp_offset}(tp)

        j {trap_handler}
        "#
        ,
        trap_handler = sym trap_handler,
        // FIXME: Add context offset in TP
        pc_offset = const offset_of!(Context, pc),
        sstatus_offset = const offset_of!(Context, sstatus),
        ra_offset = const offset_of!(Context, ra),
        sp_offset = const offset_of!(Context, sp),
        gp_offset = const offset_of!(Context, gp),
        tp_offset = const offset_of!(Context, tp),
        t0_offset = const offset_of!(Context, t0),
        t1_offset = const offset_of!(Context, t1),
        t2_offset = const offset_of!(Context, t2),
        t3_offset = const offset_of!(Context, t3),
        t4_offset = const offset_of!(Context, t4),
        t5_offset = const offset_of!(Context, t5),
        t6_offset = const offset_of!(Context, t6),
        a0_offset = const offset_of!(Context, a0),
        a1_offset = const offset_of!(Context, a1),
        a2_offset = const offset_of!(Context, a2),
        a3_offset = const offset_of!(Context, a3),
        a4_offset = const offset_of!(Context, a4),
        a5_offset = const offset_of!(Context, a5),
        a6_offset = const offset_of!(Context, a6),
        a7_offset = const offset_of!(Context, a7),
        s0_offset = const offset_of!(Context, s0),
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
        options(noreturn),
    );
}

pub unsafe fn switch_to_user(context: &Context) -> ! {
    println!("switch_to_user: pc={:x}", context.pc);
    core::arch::asm!(
        r#"
        csrw sepc, {user_pc}
        csrw sstatus, {sstatus}
        csrw sscratch, {user_tp}
        ld ra, {ra_offset}(tp)
        // ld sp, {sp_offset}(tp) FIXME:
        ld gp, {gp_offset}(tp)
        ld t0, {t0_offset}(tp)
        ld t1, {t1_offset}(tp)
        ld t2, {t2_offset}(tp)
        ld t3, {t3_offset}(tp)
        ld t4, {t4_offset}(tp)
        ld t5, {t5_offset}(tp)
        ld t6, {t6_offset}(tp)
        ld a0, {a0_offset}(tp)
        ld a1, {a1_offset}(tp)
        ld a2, {a2_offset}(tp)
        ld a3, {a3_offset}(tp)
        ld a4, {a4_offset}(tp)
        ld a5, {a5_offset}(tp)
        ld a6, {a6_offset}(tp)
        ld a7, {a7_offset}(tp)
        ld s0, {s0_offset}(tp)
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
        csrrw tp, sscratch, tp
        sret
        "#,
        user_pc = in(reg) context.pc,
        sstatus = in(reg) context.sstatus,
        user_tp = in(reg) context.tp,
        // FIXME: Add context offset in TP
        ra_offset = const offset_of!(Context, ra),
        sp_offset = const offset_of!(Context, sp),
        gp_offset = const offset_of!(Context, gp),
        t0_offset = const offset_of!(Context, t0),
        t1_offset = const offset_of!(Context, t1),
        t2_offset = const offset_of!(Context, t2),
        t3_offset = const offset_of!(Context, t3),
        t4_offset = const offset_of!(Context, t4),
        t5_offset = const offset_of!(Context, t5),
        t6_offset = const offset_of!(Context, t6),
        a0_offset = const offset_of!(Context, a0),
        a1_offset = const offset_of!(Context, a1),
        a2_offset = const offset_of!(Context, a2),
        a3_offset = const offset_of!(Context, a3),
        a4_offset = const offset_of!(Context, a4),
        a5_offset = const offset_of!(Context, a5),
        a6_offset = const offset_of!(Context, a6),
        a7_offset = const offset_of!(Context, a7),
        s0_offset = const offset_of!(Context, s0),
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
        options(noreturn),
    );
}
