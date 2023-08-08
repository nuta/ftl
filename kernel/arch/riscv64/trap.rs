use core::arch::asm;

use crate::{
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

    if scause == 8 {
        println!("system call");
        let mut cpuvar = cpuvar_mut();
        let current = cpuvar
            .current_thread
            .as_mut()
            .expect("no current thread");

        println!("checking pc");
        loop {}
        println!("pc={:x}", current.borrow_mut().context_mut().pc);
        current.borrow_mut().context_mut().pc += 4;

        yield_to_user();
    }

    println!("trap_handler: scause={:x}, sepc={:x}", scause, sepc,);

    panic!();
}
