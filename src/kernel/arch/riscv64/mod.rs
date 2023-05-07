use riscv::{
    instructions::{rdcycle, wfi},
    sbi,
};

core::arch::global_asm!(include_str!("boot.S"));

mod boot;
mod switch;

pub fn read_cpu_cycles() -> usize {
    rdcycle() as usize
}

pub fn shutdown() {
    sbi::shutdown();
}

pub fn hang() -> ! {
    loop {
        wfi();
    }
}

pub fn console_write(bytes: &[u8]) {
    for b in bytes {
        // Ignore errors. We can't do anything if something goes wrong
        // anyway.
        let _ = sbi::console_putchar(*b);
    }
}
