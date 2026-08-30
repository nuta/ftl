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

use ftl::syscall::net_acquire;
use ftl::syscall::poll_create;
use ftl_types::handle::HandleId;
use ftl_types::net::NET_IPV4;
use ftl_types::net::NET_LISTEN;
use ftl_types::net::NET_TCP;

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
    let container =
        Container::new(root_isolate, root_vmspace, hello_elf).expect("failed to start LX");
    let net_poll = poll_create().expect("failed to create network poll");
    let net_handle = net_acquire(net_poll, NET_IPV4 | NET_TCP | NET_LISTEN, 0, 80)
        .expect("failed to acquire TCP listener network");
    let network = net::NetworkService::new(net_poll, net_handle);
    container.set_network(network.clone());
    network.run();
}
