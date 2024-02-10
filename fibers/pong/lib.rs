#![no_std]

use ftl_api::println;

pub fn main() {
    println!("fiber B: world");
    for i in 0.. {
        ftl_api::thread::yield_cpu();
        println!("fiber B: {}", i);
    }
}
