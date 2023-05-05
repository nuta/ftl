use core::arch::asm;

pub fn rdcycle() -> u64 {
    let mut cycles: u64;
    unsafe {
        asm!("rdcycle {}", out(reg) cycles);
    }
    cycles
}
