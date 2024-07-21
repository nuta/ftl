use core::arch::asm;

use super::tss::Tss;

pub const TSS_SEG: u16 = 48;

const GDT_TEMPLATE: [u64; 8] = [
    0x0000000000000000, // null
    0x00af9a000000ffff, // kernel_cs
    0x00af92000000ffff, // kernel_ds
    0x0000000000000000, // user_cs32
    0x008ff2000000ffff, // user_ds
    0x00affa000000ffff, // user_cs64
    0,                  // tss_low
    0,                  // tss_high
];

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

pub struct Gdt {
    entries: [u64; 8],
}

impl Gdt {
    pub fn new() -> Self {
        Self { entries: GDT_TEMPLATE }
    }

    // TODO: Make sure `self` is at a fixed address.
    pub fn load(&mut self, tss: &Tss) {
        // Fill the TSS descriptor.
        let tss_addr = tss as *const _ as u64;
        self.entries[(TSS_SEG as usize) / 8] = 0x0000890000000000
            | (size_of_val(&self.entries) as u64)
            | ((tss_addr & 0xffff) << 16)
            | (((tss_addr >> 16) & 0xff) << 32)
            | (((tss_addr >> 24) & 0xff) << 56);
        self.entries[(TSS_SEG as usize) / 8 + 1] = tss_addr >> 32;

        let base = &self.entries as *const _ as u64;
        let limit = (self.entries.len() * size_of::<u64>() - 1).try_into().unwrap();
        let gdtr = Gdtr { limit, base };

        unsafe {
            asm!("lgdt [{}]", in(reg) &gdtr);
        }
    }
}
