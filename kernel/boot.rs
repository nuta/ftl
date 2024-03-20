use arrayvec::ArrayVec;

use crate::arch;
use crate::memory;

/// A free region of memory available for software.
#[derive(Debug)]
pub struct FreeMem {
    /// The start address of the region.
    pub start: usize,
    /// The size of the region.
    pub size: usize,
}

/// The boot information passed from the bootloader.
#[derive(Debug)]
pub struct BootInfo {
    pub free_mems: ArrayVec<FreeMem, 8>,
    pub dtb_addr: *const u8,
}

/// The entry point of the kernel.
pub fn boot(cpu_id: usize, bootinfo: BootInfo) -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    memory::init(&bootinfo);
    arch::init(cpu_id);

    loop {
        println!("idle loop");
        arch::idle();
    }
}
