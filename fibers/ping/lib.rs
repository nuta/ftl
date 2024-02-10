#![no_std]

use ftl_api::println;

pub fn main() {
    println!("fiber A: hello");
    for i in 0.. {
        crate::arch::yield_cpu();
        println!("fiber A: {}", i);
    }
}
