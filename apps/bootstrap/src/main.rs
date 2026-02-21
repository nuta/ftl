#![no_std]
#![no_main]

use ftl::prelude::*;

mod elf;
mod initfs;
mod loader;

#[ftl::main]
fn main() {
    info!("Hello from bootstrap!");
    let initfs = initfs::InitFs::from_start_info();
    loader::load(&initfs);
}
