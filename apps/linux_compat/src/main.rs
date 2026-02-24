#![no_std]
#![no_main]
#![allow(unused)]

use ftl::prelude::*;

mod process;
mod thread;

#[ftl::main]
fn main() {
    info!("Hello, world!");
}
