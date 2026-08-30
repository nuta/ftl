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
use core::num::NonZeroU16;

use ftl::syscall::net_bind;
use ftl::syscall::net_create;
use ftl::syscall::net_subscribe;
use ftl::syscall::poll_create;
use ftl::syscall::poll_wait;
use ftl_types::handle::HandleId;
use ftl_types::net::NetMatch;

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
    let poll = poll_create().expect("failed to create poll");
    let net_handle = net_create().expect("failed to create network");
    let listener_cookie = 1;
    let listener_rule = NetMatch::tcp_ipv4_listener(None, NonZeroU16::new(80).unwrap());
    net_bind(net_handle, &listener_rule, listener_cookie)
        .expect("failed to bind TCP listener rule");
    let network = net::TcpIp::new(net_handle, listener_cookie);
    container.set_network(network.clone());
    net_subscribe(net_handle, poll).expect("failed to subscribe to network events");
    loop {
        let event = poll_wait(poll).expect("poll wait failed");
        if event.handle_id() == net_handle {
            network.handle_event();
            net_subscribe(net_handle, poll).expect("failed to subscribe to network events");
        }
    }
}
