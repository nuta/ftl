use core::arch::asm;

use bitflags::bitflags;

#[repr(usize)]
pub enum TrapMode {
    Direct = 0,
}

pub struct Stvec;

impl Stvec {
    pub unsafe fn write(addr: usize, mode: TrapMode) {
        assert!(addr & 0b11 == 0, "addr is not aligned");
        asm!(
                "csrw stvec, {}",
            in(reg) (addr | mode as usize),
        );
    }
}

pub struct Sepc;

impl Sepc {
    pub unsafe fn write(addr: usize) {
        asm!(
                "csrw sepc, {}",
            in(reg) addr,
        );
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SstatusFlags: usize {
        const UIE = 1 << 0;
        const SIE = 1 << 1;
        const UPIE = 1 << 4;
        const SPIE = 1 << 5;
        const SPP = 1 << 8;
    }
}

pub struct Sstatus;

impl Sstatus {
    pub unsafe fn write(flags: SstatusFlags) {
        asm!(
                "csrw sstatus, {}",
            in(reg) flags.bits(),
        );
    }

    pub fn read() -> SstatusFlags {
        let flags: usize;
        unsafe {
            asm!(
                    "csrr {}, sstatus",
                out(reg) flags,
            );
        }
        SstatusFlags::from_bits_retain(flags)
    }
}
