use arrayvec::ArrayVec;

use crate::{
    arch, autopilot, memory,
    scheduler::{Scheduler, GLOBAL_SCHEDULER},
};

/// A free region of memory available for software.
#[derive(Debug)]
pub struct FreeMem {
    /// The start address of the region.
    pub start: usize,
    /// The size of the region.
    pub size: usize,
}

#[derive(Debug)]
pub struct BootInfo {
    pub free_mems: ArrayVec<FreeMem, 8>,
    pub kernel_fibers: &'static [(&'static str, fn(*const i8))],
    pub dtb_addr: *const u8,
}

pub fn boot(bootinfo: BootInfo) -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    memory::init(&bootinfo);
    arch::init();

    autopilot::start(&bootinfo);

    arch::yield_cpu();

    loop {
        println!("idle loop");
        arch::idle();
    }
}
