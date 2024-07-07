use ftl_inlinedvec::InlinedVec;
use ftl_types::spec::Spec;
use ftl_types::spec::SpecFile;
use ftl_utils::byte_size::ByteSize;

use crate::arch;
use crate::autopilot::Autopilot;
use crate::bootfs::Bootfs;
use crate::cpuvar;
use crate::cpuvar::CpuId;
use crate::device_tree::walk_device_nodes;
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
    pub free_mems: InlinedVec<FreeMem, 8>,
    pub dtb_addr: *const u8,
}

pub fn console_write(bytes: &[u8]) {
    let ptr: *mut u8 = 0x40000000 as *mut u8;
    for byte in bytes {
        unsafe {
            core::ptr::write_volatile(ptr, *byte);
        }
    }
}

#[inline(never)]
#[no_mangle]
fn bar(mut printer: crate::print::Printer, args: core::fmt::Arguments) {
    use core::fmt::Write;
    let _ = printer.write_fmt(args);
}

#[inline(never)]
#[no_mangle]
fn baz(a: &[u8; 128], b: &mut [u8; 128]) {
    unsafe { core::ptr::copy_nonoverlapping(a, b, 128); }
}

#[no_mangle]
// #[naked]
pub fn asm_print() {
    use core::fmt::Write;

    console_write(b"memcpy!!!\n");
    let a: [u8; 128] = [0; 128];
    let mut b: [u8; 128] = [0; 128];
    baz(&a, &mut b);

    console_write(b"Printer::write_str\n");
    let mut printer = crate::print::Printer;
    let _ = printer.write_str("hello from printer\n");

    console_write(b"Arguments::new\n");
    let args = core::fmt::Arguments::new_const(&[]);

    console_write(b"Printer::write_fmt\n");
    let _ = printer.write_fmt(args);

    console_write(b"Arguments::new (2)\n");
    let args2 = core::fmt::Arguments::new_const(&["a"]);

    console_write(b"Printer::write_fmt (2)\n");
    bar(printer, args2);

    console_write(b"asm_print done\n");
    // core::arch::asm!(
    //     r#"
    //         sub sp, sp, #0x90
    //         stp x29, x30, [sp, #0x80]
    //         add x29, sp, #0x80
    //         nop
    //         adr x0, 0x136aa0
    //         add x8, sp, #0x20
    //         str x8, [sp, #0x8]
    //         bl {arguments_new} // 0x11abd4 <core::fmt::Arguments::new_const::h4274c0459f56a712>
    //         ldr x1, [sp, #0x8]
    //         mov w8, #0x30               // =48
    //         mov w2, w8
    //         sub x0, x29, #0x30
    //         str x0, [sp, #0x10]
    //         bl  0x12fa50 <memcpy>
    //         ldr x1, [sp, #0x10]
    //         add x0, sp, #0x1f
    //         bl  {write_fmt} // 0x947d4 <core::fmt::Write::write_fmt::hdac2d0276e50ecf9>
    //         ldp x29, x30, [sp, #0x80]
    //         add sp, sp, #0x90
    //         ret
    //     "#,
    //     arguments_new = sym core::fmt::Arguments::new_const,
    //     write_fmt = sym core::fmt::Write::write_fmt
    // )
}

#[no_mangle]
#[inline(never)]
fn foo () {
    use core::fmt::Write;
    let mut printer = crate::print::Printer;
    let args = format_args!("123");
    let _ = printer.write_fmt(args);
}

/// The entry point of the kernel.
pub fn boot(cpu_id: CpuId, bootinfo: BootInfo) -> ! {
    // console_write(b"before foo");
    // foo();
    asm_print();
    // console_write(b"after foo");

    println!("\nFTL - Faster Than \"L\"\n");

    memory::init(&bootinfo);
    arch::init();
    process::init();
    cpuvar::percpu_init(cpu_id);

    let devices = walk_device_nodes(bootinfo.dtb_addr);
    for device in &devices {
        println!("device: {} ({})", device.compatible, device.name);
    }

    let bootfs = Bootfs::load();
    for file in bootfs.files() {
        println!("bootfs: file: {}", file.name);
    }

    let boot_spec_file = bootfs.find_by_name("cfg/boot.spec.json").expect("boot spec not found");
    let spec_file: SpecFile = serde_json::from_slice(&boot_spec_file.data)
        .expect("failed to parse boot spec");
    let boot_spec = match spec_file.spec {
        Spec::Boot(boot_spec) => boot_spec,
        _ => panic!("unexpected boot spec"),
    };

    let mut autopilot = Autopilot::new();
    autopilot.boot(&bootfs, &boot_spec, &devices);

    arch::yield_cpu();

    panic!("halt");
}
