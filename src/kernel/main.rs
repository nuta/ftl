#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(fn_align)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

use riscv::{
    registers::{Stvec, TrapMode},
    sbi,
};

core::arch::global_asm!(include_str!("boot.S"));

#[macro_use]
mod print;

mod panic;
mod switch;
mod test;

#[no_mangle]
pub fn rust_entry() {
    #[cfg(test)]
    test_main();

    println!();
    unsafe {
        let handler_addr = switch::switch_to_kernel as *const () as usize;
        Stvec::write(handler_addr, TrapMode::Direct);
    }

    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");

    unsafe {
        sbi::shutdown();
    }
}
