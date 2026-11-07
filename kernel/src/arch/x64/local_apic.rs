use ftl_types::time::Duration;

use super::io_apic::IRQ_VECTOR_BASE;
use super::msr::rdmsr;
use super::paddr2vaddr;
use super::timer::TIMER_HZ;
use super::timer::TIMER_IRQ;
use super::timer::monotime_read;
use crate::address::PAddr;
use crate::address::VAddr;

const MSR_IA32_APIC_BASE: u32 = 0x1b;
pub(super) const SPURIOUS_INTERRUPT_VECTOR: u8 = 0xff;

const TIMER_DIVIDE_BY_16: u32 = 0b0011;
const TIMER_MASKED: u32 = 1 << 16;
const TIMER_PERIODIC: u32 = 1 << 17;
const TIMER_CALIBRATION_DURATION: Duration = Duration::from_millis(10);

fn read(base: VAddr, reg: Reg) -> u32 {
    let addr = (base.as_usize() + reg as usize) as *const u32;
    unsafe { core::ptr::read_volatile(addr) }
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
    EndOfInterrupt = 0xb0,
    SpuriousInterruptVector = 0xf0,
    LvtTimer = 0x320,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivideConfig = 0x3e0,
}

fn measure_lapic_frequency(base: VAddr) -> u64 {
    write(base, Reg::TimerDivideConfig, TIMER_DIVIDE_BY_16);
    write(base, Reg::LvtTimer, TIMER_MASKED);
    write(base, Reg::TimerInitialCount, u32::MAX);

    let start = monotime_read();
    let elapsed = loop {
        let elapsed = monotime_read().as_nanos() - start.as_nanos();
        if elapsed >= TIMER_CALIBRATION_DURATION.as_nanos() {
            break elapsed;
        }

        core::hint::spin_loop();
    };

    let elapsed_ticks = (u32::MAX - read(base, Reg::TimerCurrentCount)) as u64;
    let ticks_per_period = elapsed_ticks * (1_000_000_000 / TIMER_HZ) / elapsed;
    ticks_per_period
}

fn start_lapic_timer(base: VAddr) {
    let ticks_per_period = measure_lapic_frequency(base);
    trace!(
        "Local APIC timer: {} ticks per {} ms",
        ticks_per_period,
        1000 / TIMER_HZ
    );

    let vector = (IRQ_VECTOR_BASE + TIMER_IRQ) as u32;
    write(base, Reg::LvtTimer, TIMER_PERIODIC | vector);
    write(base, Reg::TimerInitialCount, ticks_per_period as u32);
}

pub struct LocalApic {
    base: VAddr,
}

impl LocalApic {
    pub fn init() -> Self {
        let base_paddr = unsafe { rdmsr(MSR_IA32_APIC_BASE) & 0xfffff000 };
        let base = paddr2vaddr(PAddr::new(base_paddr as usize));
        trace!("Local APIC base: {:x}", base_paddr);

        // Accept all interrupts.
        write(base, Reg::TaskPriority, 0);
        // Enable APIC and route spurious interrupts.
        write(
            base,
            Reg::SpuriousInterruptVector,
            (1 << 8) | (SPURIOUS_INTERRUPT_VECTOR as u32),
        );

        start_lapic_timer(base);
        Self { base }
    }

    pub fn acknowledge_irq(&self) {
        write(self.base, Reg::EndOfInterrupt, 0);
    }
}
