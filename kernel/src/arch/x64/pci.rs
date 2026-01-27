use ftl_types::pci::PciEntry;

pub fn get_pci_devices(vendor: u16, device: u16) {
    for bus in 0..256 {
        for slot in 0..32 {
            println!("bus: {}, slot: {}", bus, slot);
        }
    }
}

pub fn sys_pci_lookup(a0: usize, a1: usize, a2: usize, a3: usize) {
    let buf = a0 as *mut PciEntry;
    let buf_len = a1 as usize;
    let vendor = a2 as u16;
    let device = a3 as u16;

    get_pci_devices(vendor, device);
}
