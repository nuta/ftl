use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;

pub struct FreeRam {
    pub start: PAddr,
    pub end: PAddr,
}

pub struct BootInfo {
    pub free_rams: ArrayVec<FreeRam, 8>,
}

pub fn boot(bootinfo: &BootInfo) -> ! {
    for ram in &bootinfo.free_rams {
        info!("free ram: {} - {}", ram.start, ram.end);
    }

    crate::memory::init();

    let mut v = alloc::collections::BTreeMap::new();
    v.insert('a', 'b');
    v.insert('c', 'd');
    v.insert('e', 'f');
    v.insert('g', 'h');
    v.insert('i', 'j');
    v.insert('k', 'l');
    v.insert('m', 'n');
    v.insert('o', 'p');
    v.insert('q', 'r');
    v.insert('s', 't');
    v.insert('u', 'v');
    v.insert('w', 'x');
    v.insert('y', 'z');
    println!("{:?}", v);

    panic!("boot complete");
}
