use ftl_types::address::PAddr;

use crate::folio::Folio;
use crate::spinlock::SpinLock;
use crate::utils::mmio::LittleEndian;
use crate::utils::mmio::MmioFolio;
use crate::utils::mmio::MmioReg;
use crate::utils::mmio::ReadWrite;

const EOI_REG: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0xb0);

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

    pub fn ack_interrupt(&mut self) {
        EOI_REG.write(&mut self.folio, 0);
    }
}

pub fn init(paddr: PAddr) {
    let folio = Folio::alloc_fixed(paddr, 0x1000).unwrap();
    LOCAL_APIC.lock().replace(LocalApic::new(folio));
}
