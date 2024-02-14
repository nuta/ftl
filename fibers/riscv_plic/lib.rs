#![no_std]

use ftl_api::{environ::Environ, println, Message};
use ftl_autogen::fibers::riscv_plic::Deps;

pub fn main(mut env: Environ) {
    println!("plic: starting: {:?}", env.device());
}
