#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

use ftl::borrow::ToOwned;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::prelude::*;

use crate::initfs::InitFs;

mod elf;
mod initfs;
mod loader;

#[ftl::main]
fn main() {
    info!("Hello from bootstrap!");
}
