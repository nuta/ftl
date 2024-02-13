#![no_std]

use ftl_api::{environ::Environ, println, Message};
use ftl_autogen::fibers::riscv_plic::Deps;

pub fn main(mut env: Environ) {
    let mut deps = env.parse_deps::<Deps>().expect("failed to parse deps");

    println!("plic: starting...");
}
