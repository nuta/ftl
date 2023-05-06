pub enum TrapMode {
    Direct = 0,
}

pub struct Stvec;

impl Stvec {
    pub unsafe fn write(addr: usize, mode: TrapMode) {
        assert!(addr & 0b11 == 0, "addr is not aligned");
        core::arch::asm!(
                "csrw stvec, {}",
            in(reg) (addr | mode as usize),
        );
    }
}
