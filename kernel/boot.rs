use core::arch::global_asm;

use ftl_inlinedvec::InlinedVec;
use ftl_types::spec::Spec;
use ftl_types::spec::SpecFile;
use ftl_utils::byte_size::ByteSize;

use crate::arch;
use crate::bootfs::Bootfs;
use crate::cpuvar;
use crate::cpuvar::CpuId;
use crate::device_tree::DeviceTree;
use crate::memory;
use crate::process;
use crate::userboot;

/// A free region of memory available for software.
#[derive(Debug)]
pub struct FreeMem {
    /// The start address of the region.
    pub start: usize,
    /// The size of the region.
    pub size: ByteSize,
}

/// The boot information passed from the bootloader.
#[derive(Debug)]
pub struct BootInfo {
    pub free_mems: InlinedVec<FreeMem, 8>,
    pub dtb_addr: *const u8,
}

/// The entry point of the kernel.
pub fn boot(cpu_id: CpuId, bootinfo: BootInfo) -> ! {
    info!("FTL - Faster Than \"L\"");

    memory::init(&bootinfo);

    let device_tree = DeviceTree::parse(bootinfo.dtb_addr);
    arch::init(&device_tree);
    process::init();
    cpuvar::percpu_init(cpu_id);

    let bootfs = Bootfs::load();
    for file in bootfs.files() {
        debug!("bootfs: file: {}", file.name);
    }

    userboot::load(&bootfs);
    arch::idle();
}
