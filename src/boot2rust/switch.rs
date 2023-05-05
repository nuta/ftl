use core::arch::asm;

#[naked]
pub unsafe extern "C" fn switch_context(prev_sp: *mut usize, next_sp: usize) {
    asm!(
        r#"
        // Save the current context.
        addi sp, sp, -13 * 8
        sd ra,  0  * 8(sp)
        sd s0,  1  * 8(sp)
        sd s1,  2  * 8(sp)
        sd s2,  3  * 8(sp)
        sd s3,  4  * 8(sp)
        sd s4,  5  * 8(sp)
        sd s5,  6  * 8(sp)
        sd s6,  7  * 8(sp)
        sd s7,  8  * 8(sp)
        sd s8,  9  * 8(sp)
        sd s9,  10 * 8(sp)
        sd s10, 11 * 8(sp)
        sd s11, 12 * 8(sp)

        sd sp, (a0) // Save prev_sp
        mv sp, a1   // Restore next_sp

        // Restore the next context.
        ld ra,  0  * 8(sp)
        ld s0,  1  * 8(sp)
        ld s1,  2  * 8(sp)
        ld s2,  3  * 8(sp)
        ld s3,  4  * 8(sp)
        ld s4,  5  * 8(sp)
        ld s5,  6  * 8(sp)
        ld s6,  7  * 8(sp)
        ld s7,  8  * 8(sp)
        ld s8,  9  * 8(sp)
        ld s9,  10 * 8(sp)
        ld s10, 11 * 8(sp)
        ld s11, 12 * 8(sp)

        addi sp, sp, 13 * 8
        ret
    "#,
        options(noreturn),
    );
}

fn trap_handler() {
    let scause: u64;
    unsafe {
        asm!("csrr {}, scause", out(reg) scause);
    }

    panic!("trap_handler: scause={:x}", scause);
}

// This function address must be aligned to 4 bytes not to accidentally set
// MODE field in stvec.
#[naked]
#[repr(align(4))]
pub unsafe extern "C" fn switch_to_kernel() -> usize {
    asm!(
        r#"
        call {trap_handler}
        "#
        ,
        trap_handler = sym trap_handler,
        options(noreturn),
    );
}
