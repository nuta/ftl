use core::arch::asm;

use riscv::registers::{Sepc, Sstatus, SstatusFlags};

fn trap_handler() {
    let scause: u64;
    unsafe {
        asm!("csrr {}, scause", out(reg) scause);
    }

    let sepc: u64;
    unsafe {
        asm!("csrr {}, sepc", out(reg) sepc);
    }

    panic!("trap_handler: scause={:x}, sepc={:x}", scause, sepc);
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

pub unsafe fn switch_to_user() {
    unsafe extern "C" fn first_user_program() {
        core::arch::asm!("nop; ecall");
    }

    Sepc::write(first_user_program as *const () as usize);

    let mut sstatus = Sstatus::read();
    // sstatus.insert(SstatusFlags::SPIE); FIXME:
    sstatus.remove(SstatusFlags::SPP);
    Sstatus::write(sstatus);

    core::arch::asm!(
        // Switch to user mode.
        "sret"
    );
    core::hint::unreachable_unchecked();
}
