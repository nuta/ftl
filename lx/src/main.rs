#![no_std]
#![no_main]

extern crate alloc;

mod arch;
mod process;
mod syscall;
mod thread;

use ftl_types::handle::HandleId;

use crate::process::Process;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static HELLO_ELF: Aligned<{ include_bytes!("../../initfs/bin/hello").len() }> =
    Aligned(*include_bytes!("../../initfs/bin/hello"));

#[unsafe(no_mangle)]
fn main() {
    let root_isolate = HandleId::new(1);
    let root_vmspace = HandleId::new(2);
    let init_process = Process::new_init(root_isolate, root_vmspace, &HELLO_ELF.0).unwrap();

    // TODO: wait for root process to exit
    core::mem::forget(init_process);
}
