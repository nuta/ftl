#![no_std]

use ftl_api::{environ::Environ, println};

pub fn main(env: Environ) {
    println!("fiber B: world");
    for i in 0.. {
        ftl_api::syscall::yield_cpu();
        println!("fiber B: {}", i);
    }
}
