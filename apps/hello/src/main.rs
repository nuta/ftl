#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use ftl::pci::PciEntry;
use ftl::println;
use ftl_utils::alignment::align_up;

mod virtio;

#[unsafe(no_mangle)]
fn main() {
    println!(
        "\x1b[1m\x1b[32mHello\x1b[0m\x1b[1m \x1b[1m\x1b[33mworld\x1b[0m\x1b[1m \x1b[1m\x1b[36mfrom\x1b[0m\x1b[1m \x1b[1m\x1b[35msystem call!\x1b[0m\x1b[1m\x1b[0m"
    );

    let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
    let n = ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1af4, 0x1000)
        .unwrap();

    let devices = unsafe { entries.assume_init() };
    println!("found {} virtio-net PCI devices", n);

    debug_assert!(n > 0, "no virtio-net device found");
    debug_assert!(n <= 1, "multiple virtio-net devices found");

    let entry = devices[0];
    println!("using device {:x}:{:x}", entry.bus, entry.slot);

    // Enable bus mastering
    ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true).unwrap();

    // Get BAR0 (I/O port base for legacy virtio)
    let bar0 = ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).unwrap();
    let iobase = (bar0 & 0xfffffffc) as u16; // Mask off the I/O space indicator bit
    println!("I/O base: {:#x}", iobase);

    // Enable IOPL for direct I/O access
    ftl::syscall::sys_x64_iopl(true).unwrap();
    println!("IOPL enabled");

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
