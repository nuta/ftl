#![no_std]
#![no_main]
use ftl::println;

#[unsafe(no_mangle)]
fn main() {
    for i in 0.. {
        println!("Hello world from system call {} times!", i);
    }
}
