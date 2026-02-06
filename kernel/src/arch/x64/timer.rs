//! Programmable Interval Timer (PIT), aka i8253/i8254.
//!
//! <https://wiki.osdev.org/Programmable_Interval_Timer>
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;

use super::ioport::out8;

pub(super) const TIMER_IRQ: u8 = 0;

/// The timer frequency in Hz. 1000 Hz = interrupt every 1ms.
const TIMER_HZ: u64 = 1000;

const PIT_CH0_DATA: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

// A well-known fixed frequency.
const PIT_HZ: u64 = 1_193_182;

const DIVISOR: u16 = (PIT_HZ / TIMER_HZ) as u16;

// The initial value is deliberately close to the max to test overflow handling
// easily.
static TICKS: AtomicU32 = AtomicU32::new(0xffff_0000);

pub(super) fn handle_interrupt() {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed);
    if ticks % 1000 == 0 {
        info!("timer tick: {}", ticks);
    }

    super::get_cpuvar().arch.local_apic.acknowledge_irq();
}

pub(super) fn init() {
    unsafe {
        let cmd = (0b11 << 4/* lobyte/hibyte */) | (0b010 << 1/* rate generator */);
        out8(PIT_COMMAND, cmd);
        out8(PIT_CH0_DATA, (DIVISOR & 0xff) as u8);
        out8(PIT_CH0_DATA, (DIVISOR >> 8) as u8);
    }
}
