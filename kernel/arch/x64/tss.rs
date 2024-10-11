use core::arch::asm;
use core::mem::offset_of;

use super::gdt::KERNEL_TSS;

#[repr(C, packed)]
pub struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_offset: u16,
    iomap: [u8; 8191],
    iomap_last_byte: u8,
}

pub const TSS: Tss = Tss {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap_offset: offset_of!(Tss, iomap) as u16,
    iomap: [0; 8191],
    iomap_last_byte: 0xff,
};

pub fn init() {
    unsafe {
        asm!("ltr ax", in("ax") KERNEL_TSS);
    }
}
