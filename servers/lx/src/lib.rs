#![cfg_attr(target_os = "none", no_std)]

use ftl_api::Spec;
use ftl_api::info;

struct Server {}

impl Server {
    fn new() -> Self {
        // TODO: Get initfs from environment
        info!("Server::new()");
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
