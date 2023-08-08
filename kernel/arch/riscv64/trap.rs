use core::arch::asm;

use crate::{
    arch::giant_lock,
    cpuvar::cpuvar_mut,
    scheduler::{self, yield_to_user},
};

// Should never return.
pub extern "C" fn trap_handler() -> ! {
    let scause: u64;
    unsafe {
        asm!("csrr {}, scause", out(reg) scause);
    }

    let sepc: u64;
    unsafe {
        asm!("csrr {}, sepc", out(reg) sepc);
    }

    giant_lock();

    if scause == 8 {
        let mut cpuvar = cpuvar_mut();
        let mut current = cpuvar
            .current_thread
            .as_mut()
            .expect("no current thread")
            .borrow_mut();
        let context = current.context_mut();

        println!("system call: a0={:x}", context.a0);
        context.pc += 4;

        drop(current);
        drop(cpuvar);

        yield_to_user();
    }

    println!("trap_handler: scause={:x}, sepc={:x}", scause, sepc,);

    panic!();
}
