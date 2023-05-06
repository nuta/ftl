#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(fn_align)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
mod print;

mod arch;
mod panic;
mod test;

pub fn kernel_main() {
    #[cfg(test)]
    test_main();

    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");
    unsafe {
        arch::shutdown();
    }
}
