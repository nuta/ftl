#![no_std]
#![no_main]
use core::mem::MaybeUninit;

use ftl::pci::PciEntry;
use ftl::println;

#[unsafe(no_mangle)]
fn main() {
    println!(
        "\x1b[1m\x1b[32mHello\x1b[0m\x1b[1m \x1b[1m\x1b[33mworld\x1b[0m\x1b[1m \x1b[1m\x1b[36mfrom\x1b[0m\x1b[1m \x1b[1m\x1b[35msystem call!\x1b[0m\x1b[1m\x1b[0m"
    );

    let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
    let n = ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1af4, 0x1000)
        .unwrap();

    let devices = unsafe { entries.assume_init() };
    println!("got {} PCI entries", n);
    for i in 0..n {
        let entry = devices[i];
        println!("{:x}:{:x}", entry.bus, entry.slot);

        println!("setting busmaster for {:x}:{:x}", entry.bus, entry.slot);
        ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true).unwrap();

        println!("getting BAR for {:x}:{:x}", entry.bus, entry.slot);
        let bar = ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).unwrap();
        println!("BAR: {:x}", bar);

        println!("allocating dmabuf");
        let mut vaddr = 0;
        let mut paddr = 0;
        ftl::dmabuf::sys_dmabuf_alloc(4096, &mut vaddr, &mut paddr).unwrap();
        println!("DMABUF: {:x} -> {:x}", vaddr, paddr);
    }

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
