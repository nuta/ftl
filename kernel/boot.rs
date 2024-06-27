use alloc::string::String;
use alloc::vec;

use arrayvec::ArrayVec;
use ftl_types::spec::AppSpec;
use ftl_types::spec::Spec;
use ftl_types::spec::SpecFile;
use ftl_utils::byte_size::ByteSize;

use crate::arch;
use crate::autopilot::Autopilot;
use crate::cpuvar;
use crate::cpuvar::CpuId;
use crate::memory;
use crate::process;

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
    pub free_mems: ArrayVec<FreeMem, 8>,
    pub dtb_addr: *const u8,
}

// const STARTUP_ELF: &[u8] = include_bytes!("../build/startup.elf");

/// The entry point of the kernel.
pub fn boot(cpu_id: CpuId, bootinfo: BootInfo) -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    memory::init(&bootinfo);
    process::init();
    cpuvar::percpu_init(cpu_id);

    fn load_app_spec(spec: &[u8], elf_file: &'static [u8]) -> (String, AppSpec, &'static [u8]) {
        let spec_file: SpecFile = serde_json::from_slice(spec).expect("failed to parse app spec");
        let Spec::App(app_spec) = spec_file.spec;
        (spec_file.name, app_spec, elf_file)
    }

    let mut autopilot = Autopilot::new();
    autopilot
        .start_apps(vec![
            load_app_spec(
                include_bytes!("../apps/ping/app.spec.json"),
                include_bytes!("../build/apps/ping.elf"),
            ),
            load_app_spec(
                include_bytes!("../apps/pong/app.spec.json"),
                include_bytes!("../build/apps/pong.elf"),
            ),
        ])
        .expect("failed to start apps");

    arch::yield_cpu();

    panic!("halt");
}
