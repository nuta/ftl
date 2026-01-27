#![no_std]
#![no_main]
use ftl::pci::PciEntry;
use ftl::println;

#[unsafe(no_mangle)]
fn main() {
    println!(
        "\x1b[1m\x1b[32mHello\x1b[0m\x1b[1m \x1b[1m\x1b[33mworld\x1b[0m\x1b[1m \x1b[1m\x1b[36mfrom\x1b[0m\x1b[1m \x1b[1m\x1b[35msystem call!\x1b[0m\x1b[1m\x1b[0m"
    );

    let mut entries = [PciEntry { bus: 0, device: 0 }; 10];
    ftl::pci::sys_pci_lookup(&mut entries, 0x1000, 0x1000).unwrap();

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
