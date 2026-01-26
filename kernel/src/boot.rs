use crate::memory;
use ftl_arrayvec::ArrayVec;
use crate::address::PAddr;

#[derive(Debug)]
pub struct FreeRam {
    pub base: PAddr,
    pub size: usize,
}

pub struct BootInfo {
    pub free_rams: ArrayVec<FreeRam, 8>,
}

pub fn boot(bootinfo: &BootInfo) -> ! {
    memory::init(bootinfo);

    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("v: {:?}", v);

    panic!("booted successfully");
}
