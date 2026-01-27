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
    ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1000, 0x1000).unwrap();

    let entryies = unsafe { entries.assume_init() };
    for i in 0.. {
        println!("entry {}: {:?}", i, entryies[i]);
    }

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
