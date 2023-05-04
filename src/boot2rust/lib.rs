#![no_std]

core::arch::global_asm!(include_str!("boot.S"));

#[macro_use]
mod print;

mod panic;
mod sbi;

#[no_mangle]
pub fn rust_entry() {
    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");
    unsafe {
        sbi::shutdown();
    }
}
