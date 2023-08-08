use crate::{arch::{write_cpuvar_addr, giant_lock} };

use super::switch::switch_to_kernel;
use riscv::{
    registers::{Stvec, TrapMode},
    sbi,
};

use core::arch::asm;

extern "C" {
    static __boot_stack_top: u8;
}

#[naked]
#[no_mangle]
#[link_section = ".boot"]
pub unsafe extern "C" fn boot() {
    asm!(
        r#"
        mv ra, zero
        mv fp, zero
        la sp, {stack_top}
        j {boot_kernel}
        "#,
        boot_kernel = sym boot_kernel,
        stack_top = sym __boot_stack_top,
        options(noreturn),
    );
}

#[no_mangle]
pub fn boot_kernel() {
    println!();
    giant_lock();

    // Mark cpuvar as invalid.
    write_cpuvar_addr(0);

    unsafe {
        Stvec::write(switch_to_kernel as *const () as usize, TrapMode::Direct);
    }

    crate::kernel_main();

    sbi::shutdown();
}
