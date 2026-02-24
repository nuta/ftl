#![no_std]
#![no_main]
#![allow(unused)]

use ftl::prelude::*;

mod elf;
mod errno;
mod fs;
mod process;
mod syscall;
mod thread;

#[ftl::main]
fn main() {
    info!("Hello, world!");
}
