#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(trim_prefix_suffix)]

extern crate alloc;

mod arch;
mod cmdline;
mod container;
mod initfs;
mod net;
mod open_file;
mod process;
mod signal;
mod syscall;
mod thread;
mod types;
mod vfs;
mod wait_queue;

use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::hspace::HandleSpace;
use ftl::net::Net;
use ftl::poll::Poll;
use ftl::vmspace::VmSpace;
use ftl_types::handle::HandleId;

use crate::container::Container;
use crate::initfs::InitFsLoader;
use crate::vfs::Console;
use crate::vfs::EmbeddedFile;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static INITFS: Aligned<{ include_bytes!("../../initfs.cpio").len() }> =
    Aligned(*include_bytes!("../../initfs.cpio"));

#[cfg(not(test))]
#[unsafe(no_mangle)]
fn main(cmdline: &[u8]) {
    let root_hspace = unsafe { HandleSpace::from_handle(HandleId::new(1)) };
    let root_vmspace = unsafe { VmSpace::from_handle(HandleId::new(2)) };

    let init =
        cmdline::parse(cmdline, b"ftl.lx.init").expect("failed to parse ftl.lx.init in cmdline");
    let argv: Vec<&[u8]> = init
        .split(|b| b.is_ascii_whitespace())
        .filter(|arg| !arg.is_empty())
        .collect();
    assert!(!argv.is_empty(), "ftl.lx.init must not be empty");

    // Open the init ELF file.
    let mut initfs = InitFsLoader::new(&INITFS.0);
    let initfs_file = initfs
        .find(|file| file.name == argv[0].trim_prefix(b"/"))
        .expect("init not found in initfs");
    let elf_file = Arc::new(EmbeddedFile::new(initfs_file.data));

    let net = Net::create().expect("failed to create network");
    let network = net::TcpIp::new(net);
    let console = Arc::new(Console::new().expect("failed to create console"));
    let _container = Container::new(
        root_hspace,
        root_vmspace,
        network.clone(),
        console.clone(),
        elf_file,
        &argv,
    )
    .expect("failed to start LX");

    let poll = Poll::create().expect("failed to create poll");
    network
        .subscribe(&poll)
        .expect("failed to subscribe to network events");
    console
        .subscribe(&poll)
        .expect("failed to subscribe to console events");

    loop {
        let event = poll.wait().expect("poll wait failed");
        if event.handle_id() == network.id() {
            network.handle_rx();
            network
                .subscribe(&poll)
                .expect("failed to subscribe to network events");
        } else if event.handle_id() == console.id() {
            console.handle_rx();
            console
                .subscribe(&poll)
                .expect("failed to subscribe to console events");
        }
    }
}
