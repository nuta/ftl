#![no_std]

use ftl_api::println;

#[link_section = ".ftl.fiber_mains"]
pub static __MAIN: extern "C" fn() = {
    extern "C" fn main() {
        println!("fiber A: hello");
        for i in 0.. {
            ftl_api::thread::yield_cpu();
            println!("fiber A: {}", i);
        }
    }

    main
};
