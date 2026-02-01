use core::ptr;

use ftl_types::error::ErrorCode;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::spinlock::SpinLock;

static IOAPIC: SpinLock<Option<IoApic>> = SpinLock::new(None);

fn read_ioapic(base: VAddr, reg: u32) -> u32 {
    unsafe {
        // IOREGSEL
        ptr::write_volatile(base.as_usize() as *mut u32, reg);
        // IOWIN
        ptr::read_volatile((base.as_usize() + 0x10) as *const u32)
    }
}

fn write_ioapic(base: VAddr, reg: u32, value: u32) {
    unsafe {
        // IOREGSEL
        ptr::write_volatile(base.as_usize() as *mut u32, reg);
        // IOWIN
        ptr::write_volatile((base.as_usize() + 0x10) as *mut u32, value);
    }
}

fn redir_reg_low(irq: u8) -> u32 {
    REDIR_TABLE_BASE + (irq as u32) * 2
}

fn redir_reg_high(irq: u8) -> u32 {
    redir_reg_low(irq) + 1
}

const REG_IOAPICVER: u32 = 0x01;
const REDIR_TABLE_BASE: u32 = 0x10;
pub(super) const IRQ_VECTOR_BASE: u8 = 32;

pub struct IoApic {
    base: VAddr,
    num_entries: u8,
}

impl IoApic {
    fn init(base: VAddr) -> Self {
        let ver = read_ioapic(base, REG_IOAPICVER);
        let num_entries = ((ver >> 16) & 0xff) as u8;
        Self { base, num_entries }
    }

    pub fn enable_irq(&mut self, irq: u8) -> Result<(), ErrorCode> {
        if irq >= self.num_entries {
            return Err(ErrorCode::OutOfBounds);
        }

        let vector = IRQ_VECTOR_BASE + irq;

        // Unkased, edge-triggered, active-high, "fixed" delivery.
        write_ioapic(self.base, redir_reg_low(irq) as u32, vector as u32);
        // Destination: BSP (APIC ID 0)
        write_ioapic(self.base, redir_reg_high(irq), 0);

        Ok(())
    }
}

pub(super) fn use_ioapic(f: impl FnOnce(&mut IoApic)) {
    let mut lock = IOAPIC.lock();
    let ioapic = lock.as_mut().expect("I/O APIC is not initialized");
    f(ioapic);
}

pub fn init(base: PAddr) {
    let mut lock = IOAPIC.lock();
    if lock.is_some() {
        println!("I/O APIC is already initialized");
        return;
    }

    println!("I/O APIC base: {}", base);
    let base = super::paddr2vaddr(base);
    let ioapic = IoApic::init(base);
    *lock = Some(ioapic);
}

pub fn interrupt_acquire(irq: u8) -> Result<(), ErrorCode> {
    let mut lock = IOAPIC.lock();
    let ioapic = lock.as_mut().expect("I/O APIC is not initialized");
    ioapic.enable_irq(irq)
}

pub fn interrupt_acknowledge(_irq: u8) {
    super::get_cpuvar().arch.local_apic.acknowledge_irq();
}
