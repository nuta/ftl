#![cfg_attr(target_os = "none", no_std)]

extern crate alloc;

use alloc::vec::Vec;

use ftl_api::Spec;
use ftl_api::info;

struct Server {}

impl Server {
    fn new() -> Self {
        // TODO: Get initfs from environment
        info!("Server::new()");
        let mut v = Vec::new();
        v.push(123);
        v.push(456);
        info!("v: {:?}", v);
        Self {}
    }
}

const HELLO_ELF: &[u8] = include_bytes!("../../../initfs/bin/hello");

fn start_init_process() -> ftl_api::Result<()> {
    // TODO: Create vmspace
    // TODO: Read ELF header
    // TODO: Copy ELF segments into vmareas
    // TODO: Create thread
    // TODO: Start thread
    Ok(())
}

#[unsafe(no_mangle)]
pub static SPEC: Spec = Spec {
    name: b"lx",
    // TODO: register the server
    start: || ftl_api::start(Server::new),
};
