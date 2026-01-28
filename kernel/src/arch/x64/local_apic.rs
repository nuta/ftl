use crate::address::PAddr;
use crate::address::VAddr;

const MSR_IA32_APIC_BASE: u32 = 0x1b;

/// Reads an MSR.
///
/// # Safety
///
/// MSR registers might have side effects.
unsafe fn rdmsr(msr: u32) -> u64 {
    let msr_low: u32;
    let msr_high: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") msr_low,
            out("edx") msr_high,
        );
    }

    ((msr_high as u64) << 32) | (msr_low as u64)
}

fn write(base: VAddr, reg: Reg, value: u32) {
    let addr = (base.as_usize() + reg as usize) as *mut u32;
    unsafe {
        core::ptr::write_volatile(addr, value);
    }
}

/// Local APIC registers.
#[repr(usize)]
enum Reg {
    TaskPriority = 0x80,
    SpuriousInterruptVector = 0xf0,
}

pub struct LocalApic {
    base: VAddr,
}

impl LocalApic {
    pub fn init() -> Self {
        let base_paddr = unsafe { rdmsr(MSR_IA32_APIC_BASE) & 0xfffff000 };
        let base = super::paddr2vaddr(PAddr::new(base_paddr as usize));
        println!("Local APIC base: {:x}", base_paddr);

        // Accept all interrupts.
        write(base, Reg::TaskPriority, 0);
        // Enable APIC.
        write(base, Reg::SpuriousInterruptVector, 1 << 8);

        Self { base }
    }
}
