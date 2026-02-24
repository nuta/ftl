#![no_std]
#![no_main]

use ftl::prelude::*;

mod process;
mod thread;

#[ftl::main]
fn main() {
    info!("Hello, world!");
}
