use ftl_types::time::WallTime;

use super::ioport::in8;
use super::ioport::out8;

// CMOS ports
const CMOS_INDEX: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

const NMI_DISABLE: u8 = 1 << 7;
const HOUR_PM: u8 = 1 << 7;

const REG_SECONDS: u8 = 0x00;
const REG_MINUTES: u8 = 0x02;
const REG_HOURS: u8 = 0x04;
const REG_DAY: u8 = 0x07;
const REG_MONTH: u8 = 0x08;
const REG_YEAR: u8 = 0x09;
const REG_STATUS_A: u8 = 0x0a;
const REG_STATUS_B: u8 = 0x0b;
const REG_CENTURY: u8 = 0x32;

#[derive(Clone, Copy, PartialEq, Eq)]
struct RtcDate {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day: u8,
    month: u8,
    year: u8,
    century: u8,
}

// Read a register from the CMOS.
fn read_reg(reg: u8) -> u8 {
    unsafe {
        out8(CMOS_INDEX, NMI_DISABLE | reg);
        in8(CMOS_DATA)
    }
}

// Returns true if the update is in progress.
fn update_in_progress() -> bool {
    read_reg(REG_STATUS_A) & (1 << 7) != 0
}

fn do_read() -> RtcDate {
    RtcDate {
        seconds: read_reg(REG_SECONDS),
        minutes: read_reg(REG_MINUTES),
        hours: read_reg(REG_HOURS),
        day: read_reg(REG_DAY),
        month: read_reg(REG_MONTH),
        year: read_reg(REG_YEAR),
        century: read_reg(REG_CENTURY),
    }
}

fn bcd_to_bin(value: u8) -> u8 {
    (value & 0x0f) + (value >> 4) * 10
}

fn decode(raw: RtcDate, status_b: u8) -> Option<WallTime> {
    let binary = status_b & (1 << 2) != 0;
    let hour_24 = status_b & (1 << 1) != 0;

    let seconds;
    let minutes;
    let day;
    let month;
    let year;
    let century;
    let mut hours;
    if binary {
        seconds = raw.seconds;
        minutes = raw.minutes;
        hours = raw.hours;
        day = raw.day;
        month = raw.month;
        year = raw.year;
        century = raw.century;
    } else {
        seconds = bcd_to_bin(raw.seconds);
        minutes = bcd_to_bin(raw.minutes);
        hours = bcd_to_bin(raw.hours & !HOUR_PM);
        day = bcd_to_bin(raw.day);
        month = bcd_to_bin(raw.month);
        year = bcd_to_bin(raw.year);
        century = bcd_to_bin(raw.century);
    }

    if !hour_24 {
        hours &= !HOUR_PM;
        if hours == 12 {
            hours = 0;
        }

        if raw.hours & HOUR_PM != 0 {
            hours += 12;
        }
    }

    WallTime::from_utc(
        century as i32 * 100 + year as i32,
        month,
        day,
        hours,
        minutes,
        seconds,
    )
}

pub(super) fn read() -> Option<WallTime> {
    let date = loop {
        let date = do_read();
        if !update_in_progress() {
            // Read again and make sure we did not read an inconsistent value.
            if date == do_read() && !update_in_progress() {
                break date;
            }
        }

        core::hint::spin_loop();
    };

    let wall = decode(date, read_reg(REG_STATUS_B));

    // Re-enable NMI.
    unsafe {
        out8(CMOS_INDEX, 0);
    }

    wall
}
