use core::hint::spin_loop;

use super::ioport::in8;
use super::ioport::out8;

fn putchar(c: u8) {
    // Wait for the serial port to be ready to receive more data.
    while unsafe { in8(COM1_LSR) } & 0x20 == 0 {
        spin_loop();
    }

    // Send the character.
    unsafe { out8(COM1_DATA, c) };
}

pub fn console_write(bytes: &[u8]) {
    for byte in bytes {
        if *byte == b'\n' {
            putchar(b'\r');
        }

        putchar(*byte);
    }
}

/// Data Register. If DLAB is set, the lower 8 bits of the divisor.
const COM1_DATA: u16 = 0x3f8;
/// Interrupt Enable Register. If DLAB is set, the upper 8 bits of the divisor.
const COM1_IER: u16 = COM1_DATA + 1;
/// FIFO Control Register.
const COM1_FCR: u16 = COM1_DATA + 2;
/// Line Control Register. DLAB is at bit 7.
const COM1_LCR: u16 = COM1_DATA + 3;
/// Modem Control Register.
const COM1_MCR: u16 = COM1_DATA + 4;
/// Line Status Register.
const COM1_LSR: u16 = COM1_DATA + 5;

pub(super) const SERIAL_IRQ: u8 = 4;

pub(super) fn handle_interrupt() {
    loop {
        let ch = unsafe { in8(COM1_DATA) };
        if ch == 0 {
            break;
        }

        trace!("serial interrupt: \x1b[1;91m{}\x1b[0m", ch as char);
    }

    let cpuvar = super::get_cpuvar();
    cpuvar.arch.local_apic.acknowledge_irq();
}

/// Initializes the serial port.
pub(super) fn init() {
    // Based on "Initialization" section in https://wiki.osdev.org/Serial_Ports
    unsafe {
        // Disable all interrupts.
        out8(COM1_IER, 0x00);
        // Set DLAB to enable access to divisor registers.
        out8(COM1_LCR, 1 << 7);
        // Set divisor to 115200 baud.
        out8(COM1_DATA, 1);
        out8(COM1_IER, 0);
        // Clear DLAB, and set 8N1 (8 data bits, no parity, 1 stop bit).
        out8(COM1_LCR, 0x03);
        // Enable FIFO, clear both TX/RX FIFOs, buffer 14 bytes in RX.
        out8(COM1_FCR, 0xc7);
        // Enable Data Terminal Ready (DTR) and Request to Send (RTS).
        out8(COM1_MCR, 0x03);
        // Enable RX interrupt.
        out8(COM1_IER, 0x01);
    }
}
