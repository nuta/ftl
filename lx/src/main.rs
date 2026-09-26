#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(trim_prefix_suffix)]

extern crate alloc;

mod arch;
mod container;
mod fd_table;
mod initfs;
mod net;
mod open_file;
mod process;
mod signal;
mod syscall;
mod thread;
mod types;
mod vfs;
mod vm;
mod wait_queue;

use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::allocator;
use ftl::hspace::HandleSpace;
use ftl::net::Net;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_types::handle::HandleId;
use ftl_types::time::Duration;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_up;

use crate::container::Container;
use crate::initfs::InitFsLoader;
use crate::vfs::Console;
use crate::vfs::EmbeddedFile;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static INITFS: Aligned<{ include_bytes!("../../initfs.cpio").len() }> =
    Aligned(*include_bytes!("../../initfs.cpio"));

/// The size of LX's heap.
const HEAP_SIZE: usize = 128 * 1024 * 1024;

unsafe extern "C" {
    /// The end of the image, defined by the linker by default.
    static _end: u8;
}

fn parse_cmdline<'a>(
    cmdline: &'a [u8],
    key: &[u8],
) -> Result<Option<&'a [u8]>, ftl_utils::cmdline::Error> {
    for param in ftl_utils::cmdline::Parser::new(cmdline) {
        let param = param?;
        if param.key == key {
            return Ok(Some(param.value));
        }
    }
    Ok(None)
}

#[unsafe(no_mangle)]
fn main(cmdline: &[u8]) {
    let root_hspace = unsafe { HandleSpace::from_handle(HandleId::new(1)) };
    let root_vmspace = unsafe { VmSpace::from_handle(HandleId::new(2)) };

    // Allocate the VMO for the heap.
    let heap_addr = align_up(&raw const _end as usize, 4096);
    let heap = Vmo::create(HEAP_SIZE).expect("failed to create heap VMO");
    root_vmspace
        .map(&heap, heap_addr, PageAttrs::READ | PageAttrs::WRITE)
        .expect("failed to map heap VMO");

    // SAFETY: The mapped heap is exclusive to the global allocator.
    unsafe { allocator::init(heap_addr as *mut u8, HEAP_SIZE) };

    let init = parse_cmdline(cmdline, b"ftl.lx.init")
        .expect("failed to parse cmdline")
        .expect("ftl.lx.init not found in cmdline");
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
        let deadline = MonoTime::now() + Duration::from_secs(1);
        let event = poll.wait_until(deadline).expect("poll wait failed");
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

        network.handle_timeouts(MonoTime::now());
    }
}
