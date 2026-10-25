#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(trim_prefix_suffix)]

extern crate alloc;

mod arch;
mod container;
mod endian;
mod fault;
mod fd_table;
mod futex;
mod initfs;
mod net;
mod open_file;
mod process;
mod random;
mod reaper;
mod signal;
mod syscall;
mod thread;
mod types;
mod vfs;
mod vm;
mod wait_queue;
mod zone;

use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::allocator;
use ftl::hspace::HandleSpace;
use ftl::net::Nic;
use ftl::quota::Quota;
use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_types::handle::HandleId;
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
    let root_vmspace = unsafe { VmSpace::from_handle(HandleId::new(1)) };
    let root_quota = unsafe { Quota::from_handle(HandleId::new(2)) };
    let image = unsafe { Vmo::from_handle(HandleId::new(3)) };
    let root_hspace = unsafe { HandleSpace::from_handle(HandleId::new(4)) };

    // Allocate the VMO for the heap.
    let heap_addr = align_up(&raw const _end as usize, 4096);
    let heap = Vmo::create_zeroed(&root_quota, HEAP_SIZE).expect("failed to create heap VMO");
    root_vmspace
        .map(
            &heap,
            heap_addr,
            0,
            HEAP_SIZE,
            PageAttrs::READ | PageAttrs::WRITE,
        )
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

    let network = match parse_cmdline(cmdline, b"ftl.lx.net").expect("failed to parse cmdline") {
        Some(b"none") => None,
        Some(b"uplink") => {
            let ports = parse_cmdline(cmdline, b"ftl.lx.ports")
                .expect("failed to parse cmdline")
                .expect("ftl.lx.ports not found in cmdline");
            let ports = core::str::from_utf8(ports).expect("invalid ftl.lx.ports");
            let (port_start, port_len) = ports.split_once(':').expect("invalid ftl.lx.ports");
            let port_start = port_start.parse().expect("invalid ftl.lx.ports");
            let port_len = port_len.parse().expect("invalid ftl.lx.ports");
            let nic = unsafe { Nic::from_handle(HandleId::new(5)) };
            Some(net::TcpIp::new(nic, None, port_start, port_len))
        }
        Some(value) => panic!("unknown ftl.lx.net: {:?}", value),
        None => {
            let pnic = unsafe { Nic::from_handle(HandleId::new(6)) };
            let nic = Nic::create_virtual().expect("failed to create NIC");
            pnic.bind(net::INIT_PORT_START, net::INIT_PORT_LEN, &nic)
                .expect("failed to bind ports");
            nic.bind(1, u16::MAX, &pnic)
                .expect("failed to bind ports");
            Some(net::TcpIp::new(
                nic,
                Some(pnic),
                net::INIT_PORT_START,
                net::INIT_PORT_LEN,
            ))
        }
    };

    let console = match parse_cmdline(cmdline, b"ftl.lx.console").expect("failed to parse cmdline")
    {
        Some(b"none") => None,
        Some(value) => panic!("unknown ftl.lx.console: {:?}", value),
        None => {
            let device = unsafe { ftl::console::Console::from_handle(HandleId::new(5)) };
            Some(Arc::new(
                Console::new(device).expect("failed to create console"),
            ))
        }
    };

    let container = Container::new(
        root_hspace,
        root_quota,
        root_vmspace,
        image,
        network,
        console,
        elf_file,
        &argv,
    )
    .expect("failed to start LX");

    container.run();
}
