#![no_std]
#![no_main]

use ftl_api::prelude::*;
use ftl_api_autogen::apps::virtio_blk::Environ;
use ftl_virtio::transports::mmio::VirtioMmio;

#[ftl_api::main]
pub fn main(_env: Environ) {
    let virtio = VirtioMmio::new(todo!());
    info!("Hello World from hello app!");
    loop {}
}
