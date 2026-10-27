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
use core::net::SocketAddrV4;
use core::str;

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
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::EmbeddedFile;
use crate::vfs::FileLike;
use crate::zone::FrameKind;
use crate::zone::StdioConn;
use crate::zone::StdioStream;

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

fn unescape(value: &[u8]) -> Vec<u8> {
    let mut unescaped = Vec::with_capacity(value.len());
    let mut bytes = value.iter();
    while let Some(&b) = bytes.next() {
        if b != b'\\' {
            unescaped.push(b);
            continue;
        }

        match bytes.next() {
            Some(b'n') => unescaped.push(b'\n'),
            Some(b'\\') => unescaped.push(b'\\'),
            _ => panic!("invalid escape in ftl.lx.arg"),
        }
    }

    unescaped
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
    let mut args = Vec::new();
    for param in ftl_utils::cmdline::Parser::new(cmdline) {
        let param = param.expect("failed to parse cmdline");
        if param.key == b"ftl.lx.arg" {
            args.push(unescape(param.value));
        }
    }

    let argv: Vec<&[u8]> = init
        .split(|b| b.is_ascii_whitespace())
        .filter(|arg| !arg.is_empty())
        .chain(args.iter().map(Vec::as_slice))
        .collect();
    assert!(!argv.is_empty(), "ftl.lx.init must not be empty");

    // Open the init ELF file.
    let mut initfs = InitFsLoader::new(&INITFS.0);
    let initfs_file = initfs
        .find(|file| file.name == argv[0].trim_prefix(b"/"))
        .expect("init not found in initfs");
    let elf_file = Arc::new(EmbeddedFile::new(initfs_file.data));

    let net = parse_cmdline(cmdline, b"ftl.lx.net").expect("failed to parse cmdline");
    let nic_id = parse_cmdline(cmdline, b"ftl.lx.nic").expect("failed to parse cmdline");
    let network = match (net, nic_id) {
        (Some(value), _) => panic!("unknown ftl.lx.net: {:?}", value),
        (None, Some(nic_id)) => {
            // This is a zone LX.

            // Parse "ftl.lx.nic".
            let nic_id = str::from_utf8(nic_id).expect("invalid ftl.lx.nic");
            let nic_id = nic_id.parse().expect("invalid ftl.lx.nic");

            // Parse "ftl.lx.ports".
            let ports_bytes = parse_cmdline(cmdline, b"ftl.lx.ports")
                .expect("failed to parse cmdline")
                .expect("ftl.lx.ports not found in cmdline");
            let ports = str::from_utf8(ports_bytes).expect("invalid ftl.lx.ports");
            let (port_min, port_max) = ports.split_once('-').expect("invalid ftl.lx.ports");
            let port_min = port_min.parse().expect("invalid ftl.lx.ports");
            let port_max = port_max.parse().expect("invalid ftl.lx.ports");

            let nic = unsafe { Nic::from_handle(HandleId::new(nic_id)) };
            net::TcpIp::new(nic, None, port_min, port_max)
        }
        (None, None) => {
            // Instantiate a NIC for the init LX.
            let pnic = unsafe { Nic::from_handle(HandleId::new(6)) };
            let nic = net::create_init_nic(&pnic, net::INIT_PORT_MIN, net::INIT_PORT_MAX)
                .expect("failed to create init NIC");

            net::TcpIp::new(
                nic,
                Some(Arc::new(pnic)),
                net::INIT_PORT_MIN,
                net::INIT_PORT_MAX,
            )
        }
    };

    let (stdio_conn, stdio) =
        match parse_cmdline(cmdline, b"ftl.lx.stdio").expect("failed to parse cmdline") {
            Some(addr) => {
                let addr = core::str::from_utf8(addr).expect("invalid ftl.lx.stdio");
                let addr: SocketAddrV4 = addr.parse().expect("invalid ftl.lx.stdio");
                let conn = network
                    .connect_tcp_to(addr)
                    .expect("failed to connect stdio");
                let conn = Arc::new(StdioConn::new(conn));
                let stdout: Arc<dyn FileLike> =
                    Arc::new(StdioStream::new(conn.clone(), FrameKind::Stdout));
                let stderr: Arc<dyn FileLike> =
                    Arc::new(StdioStream::new(conn.clone(), FrameKind::Stderr));
                (Some(conn), Some((stdout, stderr)))
            }
            None => (None, None),
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
        stdio,
        elf_file,
        &argv,
    )
    .expect("failed to start LX");

    // Keep LX running until the init process exits.
    let status = loop {
        container.handle_events();
        if let Some(status) = container.init_exit_status() {
            break status;
        }
    };

    ftl::trace!("init process exited with status {}", status);

    // The init process has exited. Report the exit status over the stdio conn.
    let Some(conn) = stdio_conn else {
        panic!("init process exited with status {}", status);
    };

    // Send the exit status over the stdio connection.
    while conn.send_exit(status) == Err(Errno::EAGAIN) {
        container.handle_events();
    }

    // Initiate a connection close.
    conn.close();

    // Keep LX running until the host LX destroys this thread.
    loop {
        container.handle_events();
    }
}
