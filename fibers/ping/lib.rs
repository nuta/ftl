#![no_std]

pub fn main() {
    println!("fiber A: hello");
    for i in 0.. {
        crate::arch::yield_cpu();
        println!("fiber A: {}", i);
    }
}
