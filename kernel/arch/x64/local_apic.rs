use core::arch::asm;

use ftl_types::address::PAddr;

use crate::folio::Folio;
use crate::spinlock::SpinLock;
use crate::utils::mmio::LittleEndian;
use crate::utils::mmio::MmioFolio;
use crate::utils::mmio::MmioReg;
use crate::utils::mmio::ReadWrite;

const EOI_REG: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0xb0);
const SPURIOUS_INT_REG: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0xf0);
const LVT_TIMER_REG: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x320);
const IA32_APIC_BASE_MSR: u32 = 0x1b;

pub static LOCAL_APIC: SpinLock<Option<LocalApic>> = SpinLock::new(None);

pub struct LocalApic {
    folio: MmioFolio,
}

impl LocalApic {
    pub fn new(folio: Folio) -> LocalApic {
        LocalApic {
            folio: MmioFolio::from_folio(folio).unwrap(),
        }
    }

    pub fn init(&mut self) {
        // Enable APIC.
        {
            let mut value = 0;
            value |= 1 << 8; // Enable APIC
            value |= self.folio.paddr().as_usize() as u64 & 0xfffff000; // APIC base address
            unsafe {
                asm!(
                    "wrmsr",
                    in("ecx") IA32_APIC_BASE_MSR,
                    in("eax") (value & 0xffffffff) as u32,
                    in("edx") ((value >> 32) & 0xffffffff) as u32,
                    options(nostack),
                )
            }
        }

        // Set spurious interrupt vector.
        {
            let mut value = SPURIOUS_INT_REG.read(&mut self.folio);
            value |= 1 << 8;
            SPURIOUS_INT_REG.write(&mut self.folio, value);
        }

        // Mask timer interrupt.
        {
            let mut value = LVT_TIMER_REG.read(&mut self.folio);
            value &= !(1 << 16); // Masked
            value |= crate::arch::x64::idt::VECTOR_IRQ_BASE + 0; // Vector
            LVT_TIMER_REG.write(&mut self.folio, value);
        }
    }

    pub fn ack_interrupt(&mut self) {
        EOI_REG.write(&mut self.folio, 0);
    }
}

pub fn init(paddr: PAddr) {
    let folio = Folio::alloc_fixed(paddr, 0x1000).unwrap();
    let mut lapic = LocalApic::new(folio);
    lapic.init();

    LOCAL_APIC.lock().replace(lapic);
}
