use core::arch::asm;

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
