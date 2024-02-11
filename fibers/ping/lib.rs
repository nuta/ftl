#![no_std]

use ftl_api::{environ::Environ, println};
use ftl_autogen::fibers::ping::Deps;

pub fn main(env: Environ) {
    let deps = env.parse_deps::<Deps>();

    println!("fiber A: hello");
    for i in 0.. {
        ftl_api::syscall::yield_cpu();
        println!("fiber A: {}", i);
    }
}
