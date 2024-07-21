use core::arch::asm;

use super::gdt::KERNEL_CS;
use super::tss::IST_RSP0;

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

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

extern "C" {
    static interrupt_handlers: [[u8; HANDLER_SIZE]; NUM_IDT_DESCS];
}

pub struct Idt {
    entries: [IdtEntry; NUM_IDT_DESCS],
}

impl Idt {
    pub fn new() -> Self {
        Self {
            entries: [IdtEntry {
                offset1: 0,
                seg: 0,
                ist: 0,
                info: 0,
                offset2: 0,
                offset3: 0,
                reserved: 0,
            }; NUM_IDT_DESCS],
        }
    }

    pub fn load(&mut self) {
        for i in 0..NUM_IDT_DESCS {
            let handler = unsafe { &interrupt_handlers[i] as *const _ as u64 };
            self.entries[i].offset1 = (handler & 0xffff) as u16;
            self.entries[i].seg = KERNEL_CS;
            self.entries[i].ist = IST_RSP0;
            self.entries[i].info = 0x8e;
            self.entries[i].offset2 = ((handler >> 16) & 0xffff) as u16;
            self.entries[i].offset3 = ((handler >> 32) & 0xffffffff) as u32;
            self.entries[i].reserved = 0;
        }

        let base = &self.entries as *const _ as u64;
        let limit = (self.entries.len() * size_of::<u64>() - 1)
            .try_into()
            .unwrap();
        let idtr = Idtr { limit, base };

        unsafe {
            asm!("lidt [{}]", in(reg) &idtr);
        }
    }
}
