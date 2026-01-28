//! Virtio device driver (legacy).
//!
//! # References
//!
//! Latest but very long:
//! <https://docs.oasis-open.org/virtio/virtio/v1.3/csd01/virtio-v1.3-csd01.html>
//!
//! Old but covers legacy + PCI concisely:
//! <https://ozlabs.org/~rusty/virtio-spec/virtio-0.9.5.pdf>

use core::arch::asm;

const PCI_IOPORT_DEVICE_FEATURES: u16 = 0;
const PCI_IOPORT_GUEST_FEATURES: u16 = 4;
const PCI_IOPORT_QUEUE_PFN: u16 = 8;
const PCI_IOPORT_QUEUE_SIZE: u16 = 12;
const PCI_IOPORT_QUEUE_SEL: u16 = 14;
const PCI_IOPORT_QUEUE_NOTIFY: u16 = 16;
const PCI_IOPORT_STATUS: u16 = 18;
const PCI_IOPORT_ISR: u16 = 19;
const PCI_IOPORT_CONFIG: u16 = 20;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_DRIVER_FAILED: u8 = 128;

pub enum Error {}

pub struct VirtioPci {
    bus: u8,
    slot: u8,
    iobase: u16,
}

impl VirtioPci {
    pub fn new(bus: u8, slot: u8, iobase: u16) -> Self {
        Self { bus, slot, iobase }
    }

    pub fn initialize1(&self) -> u32 {
        // 1. Reset the device. This is not required on initial start up.
        // 2. The ACKNOWLEDGE status bit is set: we have noticed the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE);

        // 3. The DRIVER status bit is set: we know how to drive the device.
        self.out8(PCI_IOPORT_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

        // 4 Device-specific setup, including reading the Device Feature Bits,
        //   discovery of virtqueues for the device, ...
        self.in32(PCI_IOPORT_DEVICE_FEATURES)
    }

    pub fn write_guest_features(&self, guest_features: u32) {
        // 5. The subset of Device Feature Bits understood by the driver is
        //    written to the device.
        self.out32(PCI_IOPORT_GUEST_FEATURES, guest_features);
    }

    pub fn initialize2(&self, guest_features: u32) {
        // 6. The DRIVER_OK status bit is set.
        self.out8(
            PCI_IOPORT_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }

    pub fn read_device_config8(&self, offset: u16) -> u8 {
        self.in8(PCI_IOPORT_CONFIG + offset)
    }

    fn out32(&self, port: u16, value: u32) {
        unsafe {
            asm!("out dx, eax", in("dx") self.iobase + port, in("eax") value);
        };
    }

    fn out8(&self, port: u16, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") self.iobase + port, in("al") value);
        };
    }

    fn in32(&self, port: u16) -> u32 {
        let value: u32;
        unsafe {
            asm!("in eax, dx", in("dx") self.iobase + port, out("eax") value);
        };
        value
    }

    fn in8(&self, port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", in("dx") self.iobase + port, out("al") value);
        };
        value
    }
}
