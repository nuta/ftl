#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use ftl::pci::PciEntry;
use ftl::println;
use ftl_utils::alignment::align_up;

use crate::virtio::PciTransport;
use crate::virtio::Virtio;

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

    const VIRTIO_NET_F_MAC: u32 = 1 << 5;
    let transport = PciTransport::new(entry.bus, entry.slot, iobase);
    let guest_features = transport.initialize1();
    transport.initialize2(guest_features);
    assert!(
        guest_features & VIRTIO_NET_F_MAC != 0,
        "MAC feature not supported"
    );
    transport.write_guest_features(guest_features);

    let mut mac = [0u8; 6];
    for i in 0..6 {
        mac[i] = transport.read_device_config8(i as u16);
    }
    println!(
        "MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
