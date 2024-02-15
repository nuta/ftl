#![no_std]

use ftl_api::{environ::Environ, println};

pub fn main(env: Environ) {
    println!("plic: starting: {:?}", env.device());
}
