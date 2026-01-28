use crate::arch::x64::ioport::out8;

/// Initializes Programmable Interrupt Controller (PIC).
///
/// <https://wiki.osdev.org/8259_PIC>
pub(super) fn init() {
    const PIC0_DATA: u16 = 0x21;
    const PIC1_DATA: u16 = 0xa1;

    // Mask all interrupts to disable legacy PIC (we use I/O APIC instead).
    unsafe {
        out8(PIC0_DATA, 0xff);
        out8(PIC1_DATA, 0xff);
    }
}
