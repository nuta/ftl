#![no_std]
#![no_main]

extern crate alloc;

mod arch;
mod container;
mod initfs;
mod net;
mod open_file;
mod process;
mod syscall;
mod thread;
mod types;
mod vfs;
mod wait_queue;

use alloc::sync::Arc;

use ftl::isolate::Isolate;
use ftl::net::Net;
use ftl::poll::Poll;
use ftl::vmspace::VmSpace;
use ftl_types::handle::HandleId;

use crate::container::Container;
use crate::initfs::InitFsLoader;
use crate::vfs::EmbeddedFile;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static INITFS: Aligned<{ include_bytes!("../../initfs.cpio").len() }> =
    Aligned(*include_bytes!("../../initfs.cpio"));

fn hello_elf() -> &'static [u8] {
    InitFsLoader::new(&INITFS.0)
        .find(|file| file.name == b"bin/hello")
        .expect("hello not found in initfs")
        .data
}

#[unsafe(no_mangle)]
fn main() {
    let root_isolate = unsafe { Isolate::from_handle(HandleId::new(1)) };
    let root_vmspace = unsafe { VmSpace::from_handle(HandleId::new(2)) };
    let hello_elf = Arc::new(EmbeddedFile::new(hello_elf()));

    let net = Net::create().expect("failed to create network");
    let network = net::TcpIp::new(net);
    let _container = Container::new(root_isolate, root_vmspace, network.clone(), hello_elf)
        .expect("failed to start LX");

    let poll = Poll::create().expect("failed to create poll");
    network
        .subscribe(&poll)
        .expect("failed to subscribe to network events");

    loop {
        let event = poll.wait().expect("poll wait failed");
        if event.handle_id() == network.id() {
            network.handle_rx();
            network
                .subscribe(&poll)
                .expect("failed to subscribe to network events");
        }
    }
}
