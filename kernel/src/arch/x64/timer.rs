//! Programmable Interval Timer (PIT), aka i8253/i8254.
//!
//! <https://wiki.osdev.org/Programmable_Interval_Timer>
use core::arch::asm;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;

use ftl_types::time::Duration;
use ftl_types::time::MonoTime;
use ftl_types::time::WallTime;

use super::ioport::in8;
use super::ioport::out8;
use super::rtc;
use crate::timer::GLOBAL_TIMER;

pub(super) const TIMER_IRQ: u8 = 0;

/// The timer frequency in Hz. 1000 Hz = interrupt every 1ms.
const TIMER_HZ: u64 = 1000;

const PIT_CH0_DATA: u16 = 0x40;
const PIT_CH2_DATA: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;
const SPEAKER_PORT: u16 = 0x61;
const SPEAKER_PIT: u8 = 1 << 0;
const SPEAKER_DATA: u8 = 1 << 1;

// A well-known fixed frequency.
const PIT_HZ: u64 = 1_193_182;

const DIVISOR: u16 = (PIT_HZ / TIMER_HZ) as u16;
const TSC_CALIBRATION_DURATION: Duration = Duration::from_millis(10);

static TSC_HZ: AtomicU64 = AtomicU64::new(0);
static UNIX_TIME_BASE_NS: AtomicU64 = AtomicU64::new(0);

fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtscp",
            out("eax") low,
            out("edx") high,
            out("ecx") _,
        );
    }
    ((high as u64) << 32) | (low as u64)
}

fn measure_tsc_frequency() {
    let calibration_count = (PIT_HZ * TSC_CALIBRATION_DURATION.as_nanos() / 1_000_000_000) as u16;
    let (start, end) = unsafe {
        let value = in8(SPEAKER_PORT);

        // Configure PIT in oneshot mode. Use the speaker channel to busy-wait
        // for the timer to fire, not for frightening humans with beeping noise.
        out8(SPEAKER_PORT, value & !(SPEAKER_PIT | SPEAKER_DATA));
        out8(PIT_COMMAND, 0xb0); // oneshot mode
        out8(PIT_CH2_DATA, calibration_count as u8);
        out8(PIT_CH2_DATA, (calibration_count >> 8) as u8);

        let start = read_tsc();

        // Start the timer, and wait for it to fire.
        out8(SPEAKER_PORT, (value & !SPEAKER_DATA) | SPEAKER_PIT);
        while in8(SPEAKER_PORT) & (1 << 5) == 0 {
            core::hint::spin_loop();
        }

        let end = read_tsc();

        // Restore the original speaker port configuration.
        out8(SPEAKER_PORT, value);
        (start, end)
    };

    TSC_HZ.store(
        (end - start) * PIT_HZ / calibration_count as u64,
        Ordering::Relaxed,
    );
}

pub(super) fn handle_interrupt() {
    // Do timekeeping job.
    GLOBAL_TIMER.lock().tick(monotime_read());

    // Acknowledge the interrupt.
    super::get_cpuvar().arch.local_apic.acknowledge_irq();
}

pub fn monotime_read() -> MonoTime {
    let hz = TSC_HZ.load(Ordering::Relaxed);
    let nanos = (read_tsc() as u128 * 1_000_000_000 / hz as u128) as u64;
    MonoTime::from_nanos(nanos)
}

pub fn walltime_read() -> WallTime {
    let base = UNIX_TIME_BASE_NS.load(Ordering::Relaxed);
    let uptime = monotime_read().as_nanos();
    WallTime::from_nanos(uptime.wrapping_add(base))
}

fn read_wall_clock() {
    let now = rtc::read().expect("invalid CMOS RTC date");
    let offset = now.as_nanos().wrapping_sub(monotime_read().as_nanos());
    UNIX_TIME_BASE_NS.store(offset, Ordering::Relaxed);
}

pub(super) fn init() {
    measure_tsc_frequency();
    read_wall_clock();

    unsafe {
        let cmd = (0b11 << 4/* lobyte/hibyte */) | (0b010 << 1/* rate generator */);
        out8(PIT_COMMAND, cmd);
        out8(PIT_CH0_DATA, (DIVISOR & 0xff) as u8);
        out8(PIT_CH0_DATA, (DIVISOR >> 8) as u8);
    }
}
