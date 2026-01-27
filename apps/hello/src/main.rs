#![no_std]
#![no_main]
use ftl::println;

#[unsafe(no_mangle)]
fn main() {
    println!("Hello world from system call!");
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
