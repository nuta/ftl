#![cfg_attr(target_os = "none", no_std)]

mod elf;
mod errno;
mod process;
mod syscall;
extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_api::Spec;
use process::Process;

struct Server {
    #[allow(unused)]
    processes: Vec<Arc<Process>>,
}

impl Server {
    fn new() -> Self {
        // TODO: Get initfs from environment
        let elf_file = HELLO_ELF.0.as_slice();
        let (process, init_regs) = Process::create(elf_file).expect("failed to create process");
        process.start(init_regs).expect("failed to start process");
        Self {
            processes: alloc::vec![process],
        }
    }
}

#[repr(align(8))]
pub struct AlignedBytes<const N: usize>([u8; N]);

pub static HELLO_ELF: AlignedBytes<{ include_bytes!("../../../initfs/bin/hello").len() }> =
    AlignedBytes(*include_bytes!("../../../initfs/bin/hello"));

#[unsafe(no_mangle)]
pub static SPEC: Spec = Spec {
    name: b"lx",
    // TODO: register the server
    start: || ftl_api::start(Server::new),
};
