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
mod lock;
mod memory;
mod panic;
mod test;

extern crate alloc;

pub fn kernel_main() {
    memory::init();

    #[cfg(test)]
    {
        test_main();
        unreachable!();
    }

    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("{:#?}", v);

    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");
    arch::shutdown();
}
