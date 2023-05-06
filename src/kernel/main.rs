#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(fn_align)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

core::arch::global_asm!(include_str!("boot.S"));

#[macro_use]
mod print;

mod asm;
mod panic;
mod sbi;
mod switch;
mod test;

#[no_mangle]
pub fn rust_entry() {
    #[cfg(test)]
    test_main();

    println!();
    unsafe {
        let handler_addr = switch::switch_to_kernel as *const () as usize;
        assert!(handler_addr & 0b11 == 0, "handler_addr is not aligned");
        // write stvec
        core::arch::asm!(
            "csrw stvec, {}",
            in(reg) (handler_addr | 0 /* direct */),
        );
    }

    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");

    unsafe {
        sbi::shutdown();
    }
}
