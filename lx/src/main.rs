#![no_std]
#![no_main]

extern crate alloc;

mod arch;
mod container;
mod net;
mod process;
mod syscall;
mod thread;
mod types;
mod vfs;

use alloc::sync::Arc;

use ftl::syscall::net_create;
use ftl::syscall::net_subscribe;
use ftl::syscall::poll_create;
use ftl::syscall::poll_wait;
use ftl_types::handle::HandleId;

use crate::container::Container;
use crate::vfs::EmbeddedFile;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static HELLO_ELF: Aligned<{ include_bytes!("../../initfs/bin/hello").len() }> =
    Aligned(*include_bytes!("../../initfs/bin/hello"));

#[unsafe(no_mangle)]
fn main() {
    let root_isolate = HandleId::new(1);
    let root_vmspace = HandleId::new(2);
    let hello_elf = Arc::new(EmbeddedFile::new(&HELLO_ELF.0));

    let net_handle = net_create().expect("failed to create network");
    let network = net::TcpIp::new(net_handle);
    let _container = Container::new(root_isolate, root_vmspace, network.clone(), hello_elf)
        .expect("failed to start LX");

    let poll = poll_create().expect("failed to create poll");
    net_subscribe(net_handle, poll).expect("failed to subscribe to network events");

    loop {
        let event = poll_wait(poll).expect("poll wait failed");
        if event.handle_id() == net_handle {
            network.handle_rx();
            net_subscribe(net_handle, poll).expect("failed to subscribe to network events");
        }
    }
}
