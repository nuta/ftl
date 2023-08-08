use core::arch::asm;

use riscv::registers::{Sstatus, SstatusFlags};

use crate::{address::UAddr, ref_count::{UniqueRef, SharedRef}, process::Process};

use super::{switch::switch_to_user, PageTable};

#[derive(Default, Debug)]
#[repr(C)]
pub struct Context {
    pub cpuvar_addr: u64,
    pub satp: u64,
    pub pc: u64,
    pub sstatus: u64,
    pub ra: u64,
    pub sp: u64,
    pub gp: u64,
    pub tp: u64,
    pub t0: u64,
    pub t1: u64,
    pub t2: u64,
    pub t3: u64,
    pub t4: u64,
    pub t5: u64,
    pub t6: u64,
    pub a0: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
    pub a7: u64,
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
}

impl Context {
    pub fn new_user(process: &SharedRef<Process>, pc: UAddr) -> Context {
        // TODO: Shoulnd't we inherit the sstatus by reading it?
        let mut sstatus = Sstatus::read();
        // sstatus.insert(SstatusFlags::SPIE);
        sstatus.remove(SstatusFlags::SPP);

        let table_paddr = UniqueRef::paddr(process.borrow_mut().page_table());
        let table_ppn = table_paddr.as_usize() >> 12;
        const SATP_SV48: usize = 9 << 60;
        let satp = SATP_SV48 | table_ppn;

        Context {
            pc: pc.as_usize() as u64,
            sstatus: sstatus.bits() as u64,
            satp: satp as u64,
            // Other registers are set to zero.
            ..Default::default()
        }
    }

    pub fn switch_to_this(&self) -> ! {
        unsafe {
            asm!(r#"
                sfence.vma zero, zero
                csrw satp, {satp}
                sfence.vma zero, zero
            "#,
                satp = in(reg) (self.satp),
            );
            switch_to_user(&self);
        }
    }
}
