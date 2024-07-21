use core::arch::asm;

const HANDLER_SIZE: usize = 16;
const NUM_IDT_DESCS: usize = 256;

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtEntry {
    offset1: u16,
    seg: u16,
    ist: u8,
    info: u8,
    offset2: u16,
    offset3: u32,
    reserved: u32,
}

pub struct Idt {
    entries: [IdtEntry; NUM_IDT_DESCS],
}

impl Idt {
    pub fn new() -> Self {
        let mut entries = [IdtEntry {
            offset1: 0,
            seg: 0,
            ist: 0,
            info: 0,
            offset2: 0,
            offset3: 0,
            reserved: 0,
        }; NUM_IDT_DESCS];

        Self { entries }
    }

    pub fn load(&self) {
        unsafe {
            asm!("lidt [{}]", in(reg) &self.entries);
        }
    }
}

