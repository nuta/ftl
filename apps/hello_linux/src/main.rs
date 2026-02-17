#![no_std]
#![no_main]

use core::hint::spin_loop;

use ftl::prelude::*;
use ftl::process::Process;
use ftl::thread::Thread;
use ftl::vmarea::VmArea;
use ftl::vmspace::PageAttrs;
use ftl::vmspace::VmSpace;

#[ftl::main]
fn main() {
    info!("starting hello_linux");

    let vmspace = VmSpace::new().unwrap();
    info!("vmspace created: {:?}", vmspace);

    let vmarea = VmArea::new(4096).unwrap();
    info!("vmarea created: {:?}", vmarea);

    vmarea.write(0, &[0xcc; 4096]).unwrap();

    vmspace.map(&vmarea, 0x10000, PageAttrs::WRITABLE).unwrap();
    info!("vmspace mapped to 0x10000");

    let process = Process::create_inkernel(&vmspace, "hello_linux").unwrap();
    info!("process created: {:?}", process);

    let thread = Thread::create(&process, 0x10000, 0, 0).unwrap();

    info!("thread created: {:?}", thread);

    thread.start().unwrap();

    info!("thread started");

    let sink = ftl::sink::Sink::new().unwrap();
    sink.wait().unwrap();
}
