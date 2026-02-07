//! Programmable Interval Timer (PIT), aka i8253/i8254.
//!
//! <https://wiki.osdev.org/Programmable_Interval_Timer>
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;

use ftl_types::time::Monotonic;

use super::ioport::out8;

pub(super) const TIMER_IRQ: u8 = 0;

/// The timer frequency in Hz. 1000 Hz = interrupt every 1ms.
const TIMER_HZ: u64 = 1000;
const NANOS_PER_TICK: u64 = 1_000_000_000 / TIMER_HZ;

const PIT_CH0_DATA: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

// A well-known fixed frequency.
const PIT_HZ: u64 = 1_193_182;

const DIVISOR: u16 = (PIT_HZ / TIMER_HZ) as u16;

// The initial value is deliberately close to the max to test overflow handling
// easily.
static TICKS: AtomicU64 = AtomicU64::new(0xffff_ffff_ffff_0000);

pub(super) fn handle_interrupt() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    crate::timer::handle_interrupt();
    super::get_cpuvar().arch.local_apic.acknowledge_irq();
}

pub fn read_timer() -> Monotonic {
    let ticks = TICKS.load(Ordering::Relaxed);
    let ns = ticks.wrapping_mul(NANOS_PER_TICK);
    Monotonic::from_nanos(ns)
}

pub fn set_timer(_deadline: Monotonic) {
    // Do nothing. PIT is not an one-shot timer.
}

pub(super) fn init() {
    unsafe {
        let cmd = (0b11 << 4/* lobyte/hibyte */) | (0b010 << 1/* rate generator */);
        out8(PIT_COMMAND, cmd);
        out8(PIT_CH0_DATA, (DIVISOR & 0xff) as u8);
        out8(PIT_CH0_DATA, (DIVISOR >> 8) as u8);
    }
}
