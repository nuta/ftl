use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_types::pci::PciEntry;

use super::ioport::in32;
use super::ioport::out32;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;

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
    carbus: u32,
    subsystem_vendor: u16,
    subsystem_id: u16,
    expansion_rom: u32,
    capabilities_pointer: u8,
    reserved: [u32; 7],
    interrupt_line: u8,
    interrupt_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

fn read_config32(bus: u8, slot: u8, offset: usize) -> u32 {
    debug_assert!(offset & 0b11 == 0, "offset must be aligned to 4 bytes");
    debug_assert!(offset < 0xff, "offset is out of range");

    let addr = (1 << 31) | ((bus as u32) << 16) | ((slot as u32) << 11) | (offset as u32);
    unsafe {
        out32(0xcf8, addr);
        in32(0xcfc)
    }
}

fn read_config16(bus: u8, slot: u8, offset: usize) -> u16 {
    debug_assert!(offset & 0b01 == 0, "offset must be aligned to 2 bytes");
    debug_assert!(offset < 0xff, "offset is out of range");

    let value = read_config32(bus, slot, offset & !0b11);
    let is_high = offset & 0b11 != 0;
    if is_high {
        (value >> 16) as u16
    } else {
        value as u16
    }
}

fn scan_one(bus: u8, slot: u8, vendor: u16, device: u16) -> Option<PciEntry> {
    if read_config16(bus, slot, offset_of!(PciConfig, vendor)) != vendor {
        return None;
    }

    if read_config16(bus, slot, offset_of!(PciConfig, device)) != device {
        return None;
    }

    let subsystem_vendor_id = read_config16(bus, slot, offset_of!(PciConfig, subsystem_vendor));
    let subsystem_id = read_config16(bus, slot, offset_of!(PciConfig, subsystem_id));

    Some(PciEntry {
        bus,
        slot,
        subsystem_vendor_id,
        subsystem_id,
    })
}

pub fn sys_pci_lookup(
    thread: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    let buf = UserPtr::new(a0);
    let n = a1;
    let vendor = a2 as u16;
    let device = a3 as u16;

    let isolation = thread.process().isolation();
    let slice = UserSlice::new(buf, n * size_of::<PciEntry>())?;
    let mut index = 0;

    'outer: for bus in 0..=255 {
        for slot in 0..32 {
            if let Some(entry) = scan_one(bus, slot, vendor, device) {
                if index >= n {
                    break 'outer;
                }

                crate::isolation::write(isolation, slice, index, entry)?;
                index += 1;
            }
        }
    }

    Ok(index)
}

pub fn sys_pci_set_busmaster(a0: usize, a1: usize, a2: usize) -> Result<usize, ErrorCode> {
    let bus = a0 as u8;
    let slot = a1 as u8;
    let enable = a2 != 0;

    // todo!("enable busmaster");
    println!("TODO: enable busmaster for {:x}:{:x}", bus, slot);
    Ok(0)
}

pub fn sys_pci_get_bar(a0: usize, a1: usize, a2: usize) -> Result<usize, ErrorCode> {
    let bus = a0 as u8;
    let slot = a1 as u8;
    let bar = a2 as u8;

    if bar >= 6 {
        return Err(ErrorCode::OutOfBounds);
    }

    let offset = offset_of!(PciConfig, bar) + (bar as usize * size_of::<u32>());
    let value = read_config32(bus, slot, offset);
    Ok(value as usize)
}
