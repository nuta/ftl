use core::mem::offset_of;
use core::mem::size_of;

use crate::env::Env;

#[repr(C, packed)]
struct PciConfig {
    vendor: u16,
    device: u16,
    command: u16,
    status: u16,
    revision: u8,
    prog_if: u8,
    subclass: u8,
    class: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    bar: [u32; 6],
    cardbus: u32,
    subsystem_vendor: u16,
    subsystem_id: u16,
    expansion_rom: u32,
    capabilities_pointer: u8,
    reserved: [u8; 7],
    interrupt_line: u8,
    interrupt_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub vendor: u16,
    pub device: u16,
    pub subsystem_vendor_id: u16,
    pub subsystem_id: u16,
}

const PCI_IOPORT_ADDR: u16 = 0xcf8;
const PCI_IOPORT_DATA: u16 = 0xcfc;

fn get_addr(bus: u8, slot: u8, offset: usize) -> u32 {
    let offset = (offset as u32) & 0xfc;
    (1 << 31) | ((bus as u32) << 16) | ((slot as u32) << 11) | offset
}

fn get_data_port16(offset: usize) -> u16 {
    PCI_IOPORT_DATA + ((offset & 0b10) as u16)
}

fn get_data_port8(offset: usize) -> u16 {
    PCI_IOPORT_DATA + ((offset & 0b11) as u16)
}

fn read_config8(env: &dyn Env, bus: u8, slot: u8, offset: usize) -> u8 {
    debug_assert!(offset < 0xff, "offset is out of range");

    unsafe {
        env.out32(PCI_IOPORT_ADDR, get_addr(bus, slot, offset));
        env.in8(get_data_port8(offset))
    }
}

fn read_config16(env: &dyn Env, bus: u8, slot: u8, offset: usize) -> u16 {
    debug_assert!(offset & 0b01 == 0, "offset must be aligned to 2 bytes");
    debug_assert!(offset < 0xff, "offset is out of range");

    unsafe {
        env.out32(PCI_IOPORT_ADDR, get_addr(bus, slot, offset));
        env.in16(get_data_port16(offset))
    }
}

fn read_config32(env: &dyn Env, bus: u8, slot: u8, offset: usize) -> u32 {
    debug_assert!(offset & 0b11 == 0, "offset must be aligned to 4 bytes");
    debug_assert!(offset < 0xff, "offset is out of range");

    unsafe {
        env.out32(PCI_IOPORT_ADDR, get_addr(bus, slot, offset));
        env.in32(PCI_IOPORT_DATA)
    }
}

fn write_config16(env: &dyn Env, bus: u8, slot: u8, offset: usize, value: u16) {
    debug_assert!(offset & 0b01 == 0, "offset must be aligned to 2 bytes");
    debug_assert!(offset < 0xff, "offset is out of range");

    unsafe {
        env.out32(PCI_IOPORT_ADDR, get_addr(bus, slot, offset));
        env.out16(get_data_port16(offset), value);
    }
}

/// Scans the PCI bus for a device matching `vendor` / `device`.
fn find_device(env: &dyn Env, vendor: u16, device: u16) -> Option<PciDevice> {
    for bus in 0..=255 {
        for slot in 0..32 {
            if read_config16(env, bus, slot, offset_of!(PciConfig, vendor)) != vendor {
                continue;
            }
            if read_config16(env, bus, slot, offset_of!(PciConfig, device)) != device {
                continue;
            }

            return Some(PciDevice {
                bus,
                slot,
                vendor,
                device,
                subsystem_vendor_id: read_config16(
                    env,
                    bus,
                    slot,
                    offset_of!(PciConfig, subsystem_vendor),
                ),
                subsystem_id: read_config16(env, bus, slot, offset_of!(PciConfig, subsystem_id)),
            });
        }
    }

    None
}

/// Scans for a virtio transitional device with the given subsystem device id.
pub fn find_virtio_device(env: &dyn Env, subsystem_id: u16) -> Option<PciDevice> {
    for device_id in 0x1000u16..=0x103f {
        if let Some(dev) = find_device(env, 0x1af4, device_id) {
            if dev.subsystem_id == subsystem_id {
                return Some(dev);
            }
        }
    }
    None
}

pub fn set_bus_master(env: &dyn Env, dev: &PciDevice, enable: bool) {
    let mut value = read_config16(env, dev.bus, dev.slot, offset_of!(PciConfig, command));
    if enable {
        value |= 1 << 2;
    } else {
        value &= !(1 << 2);
    }
    write_config16(
        env,
        dev.bus,
        dev.slot,
        offset_of!(PciConfig, command),
        value,
    );
}

pub fn get_bar(env: &dyn Env, dev: &PciDevice, bar: u8) -> u32 {
    assert!(bar < 6);
    let offset = offset_of!(PciConfig, bar) + (bar as usize * size_of::<u32>());
    read_config32(env, dev.bus, dev.slot, offset)
}

pub fn get_interrupt_line(env: &dyn Env, dev: &PciDevice) -> u8 {
    read_config8(
        env,
        dev.bus,
        dev.slot,
        offset_of!(PciConfig, interrupt_line),
    )
}
