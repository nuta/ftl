//! MultiProcessor Specification Version 1.4.
//!
//! <https://web.archive.org/web/20121002210153/http://download.intel.com/design/archives/processors/pro/docs/24201606.pdf>

use core::ops::Range;

use super::io_apic;
use crate::address::PAddr;
use crate::arch::x64::io_apic::use_ioapic;
use crate::arch::x64::timer::TIMER_IRQ;

/// The MP floating pointer table.
#[derive(Debug)]
#[repr(C, packed)]
pub struct MpPointerTable {
    /// `"_MP_"`.
    signature: [u8; 4],
    /// The physical address of the the MP configuration table.
    physical_addr_pointer: u32,
    /// The length of the MP configuration table in 16-byte units.
    length: u8,
    /// The version of the MP specification. `0x04` indicates version 1.4.
    spec_rev: u8,
    /// The checksum of this table.
    checksum: u8,
    /// The MP feature info. See the spec.
    mp_feature_bytes: [u8; 5],
}

/// The MP configuration table.
#[derive(Debug)]
#[repr(C, packed)]
pub struct MpConfigTable {
    /// `"PCMP"`.
    signature: [u8; 4],
    /// The length of this table in bytes.
    length: u16,
    /// The version of the MP specification. `0x04` indicates version 1.4.
    spec_rev: u8,
    /// The checksum of this table.
    checksum: u8,
    /// The OEM ID string.
    oem_id: [u8; 8],
    /// The product ID string.
    product_id: [u8; 12],
    /// The OEM-defined configuration table pointer. Optional.
    oem_table_pointer: u32,
    /// The size of the OEM-defined configuration table.
    oem_table_size: u16,
    /// The number of entries in this table.
    entry_count: u16,
    /// The local APIC physical address.
    local_apic_pointer: u32,
    /// The size of the extended table entries in bytes.
    extended_table_length: u16,
    /// The checksum of extended table entries.
    extended_table_checksum: u8,
    /// Reserved.
    reserved: u8,
}

/// The processor entry.
#[derive(Debug)]
#[repr(C, packed)]
pub struct ProcessorEntry {
    /// The entry type (`0` for processor entry).
    entry_type: u8,
    /// The local APIC ID.
    local_apic_id: u8,
    /// The local APIC version (0-7 bits).
    local_apic_version: u8,
    /// CPU status. If [0:3] is zero, it is unusable.
    cpu_flags: u8,
    /// The CPU family and model.
    cpu_signature: u32,
    /// CPU features. You should be able to use CPUID instead.
    feature_flags: u32,
    /// Reserved for future use.
    reserved: [u8; 8],
}

const ENTRY_TYPE_PROCESSOR: u8 = 0;
const ENTRY_TYPE_BUS: u8 = 1;
const ENTRY_TYPE_IO_APIC: u8 = 2;
const ENTRY_TYPE_IO_INT_ASSIGN: u8 = 3;

/// The bus entry.
#[derive(Debug)]
#[repr(C, packed)]
pub struct BusEntry {
    /// The entry type (`1` for bus entry).
    entry_type: u8,
    /// The bus ID.
    bus_id: u8,
    /// The bus type.
    bus_string: [u8; 6],
}

/// The I/O APIC entry.
#[derive(Debug)]
#[repr(C, packed)]
pub struct IoApicEntry {
    /// The entry type (`2` for I/O APIC entry).
    entry_type: u8,
    /// The I/O APIC ID.
    io_apic_id: u8,
    /// The I/O APIC version.
    io_apic_version: u8,
    /// The I/O APIC flags. If zero, it is unusable.
    io_apic_flags: u8,
    /// The I/O APIC physical address.
    io_apic_address: u32,
}

/// The I/O interrupt assignment entry.
#[derive(Debug)]
#[repr(C, packed)]
pub struct IoInterruptAssignmentEntry {
    /// The entry type (`3` for I/O interrupt assignment entry).
    entry_type: u8,
    /// The interrupt type. 0 for an interrupt. 1 for an NMI.
    interrupt_type: u8,
    /// The flags that we don't care about.
    flag: u16,
    /// The source bus ID.
    source_bus_id: u8,
    /// The source bus IRQ.
    source_bus_irq: u8,
    /// The destination I/O APIC's ID.
    dest_io_apic_id: u8,
    /// The destination pin in the I/O APIC.
    dest_io_apic_intin: u8,
}

unsafe fn paddr2ptr<T>(paddr: usize) -> &'static T {
    let vaddr = super::paddr2vaddr(PAddr::new(paddr));
    unsafe { &*(vaddr.as_usize() as *const T) }
}

fn find_mpfp_table() -> Option<&'static MpPointerTable> {
    // > This structure must be stored in at least one of the following memory
    // > locations, because the operating system searches for the MP floating
    // > pointer structure in the order described below:
    const SEARCH_LOCATIONS: [Range<usize>; 2] = [
        // > a. In the first kilobyte of Extended BIOS Data Area (EBDA), or
        0x80000..0x81000,
        // Ignored this case:
        //
        // > b. Within the last kilobyte of system base memory (e.g., 639K-640K for
        // >    systems with 640 KB of base memory or 511K-512K for systems with
        // >    512 KB of base memory) if the EBDA segment is undefined, or

        // > c. In the BIOS ROM address space between 0F0000h and 0FFFFFh
        0xf0000..0x100000,
    ];

    for range in SEARCH_LOCATIONS {
        for paddr in range.step_by(4) {
            let magic = unsafe { paddr2ptr::<[u8; 4]>(paddr) };
            if magic == b"_MP_" {
                return Some(unsafe { paddr2ptr::<MpPointerTable>(paddr) });
            }
        }
    }

    None
}

enum MpTableEntry<'a> {
    Processor(&'a ProcessorEntry),
    Bus(&'a BusEntry),
    IoApic(&'a IoApicEntry),
    IoInterruptAssignment(&'a IoInterruptAssignmentEntry),
    Unknown(u8 /* type */),
}

#[derive(Clone)]
struct MpTableIter<'a> {
    mp_config: &'a MpConfigTable,
    index: u16,
    entry_addr: usize,
}

impl<'a> MpTableIter<'a> {
    fn new(mp_table: &'a MpPointerTable) -> Self {
        let mp_config_addr = mp_table.physical_addr_pointer as usize;
        let mp_config = unsafe { paddr2ptr::<MpConfigTable>(mp_config_addr) };
        let entry_addr = mp_config_addr + size_of::<MpConfigTable>();
        Self {
            mp_config,
            index: 0,
            entry_addr,
        }
    }
}

impl<'a> Iterator for MpTableIter<'a> {
    type Item = MpTableEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.mp_config.entry_count {
            return None;
        }

        let type_byte = unsafe { paddr2ptr::<u8>(self.entry_addr) };
        let (entry, entry_size) = match *type_byte {
            ENTRY_TYPE_PROCESSOR => {
                let entry = unsafe { paddr2ptr::<ProcessorEntry>(self.entry_addr) };
                (MpTableEntry::Processor(entry), 20)
            }
            ENTRY_TYPE_BUS => {
                let entry = unsafe { paddr2ptr::<BusEntry>(self.entry_addr) };
                (MpTableEntry::Bus(entry), 8)
            }
            ENTRY_TYPE_IO_APIC => {
                let entry = unsafe { paddr2ptr::<IoApicEntry>(self.entry_addr) };
                (MpTableEntry::IoApic(entry), 8)
            }
            ENTRY_TYPE_IO_INT_ASSIGN => {
                let entry = unsafe { paddr2ptr::<IoInterruptAssignmentEntry>(self.entry_addr) };
                (MpTableEntry::IoInterruptAssignment(entry), 8)
            }
            _ => {
                // trace!("unknown entry type: {}", type_byte);
                (MpTableEntry::Unknown(*type_byte), 8)
            }
        };

        self.entry_addr += entry_size;
        self.index += 1;
        Some(entry)
    }
}

/// Loads the MP configuration table to locate CPUs and I/O APICs.
pub fn init() {
    let mp_table = find_mpfp_table().expect("failed to locate MP floating pointer table");
    let iter = MpTableIter::new(mp_table);

    // Find the ISA bus and I/O APIC.
    let mut isa_bus = None;
    let mut ioapic = None;
    for entry in iter.clone() {
        match entry {
            MpTableEntry::Bus(entry) => {
                if entry.bus_string.as_slice() == b"ISA   " {
                    assert!(isa_bus.is_none(), "multiple ISA buses found");
                    isa_bus = Some(entry);
                }
            }
            MpTableEntry::IoApic(entry) => {
                assert!(ioapic.is_none(), "multiple I/O APICs found");
                ioapic = Some(entry);
            }
            _ => {}
        }
    }

    let isa_bus = isa_bus.expect("ISA bus not found");
    let ioapic = ioapic.expect("I/O APIC not found");

    // Locate the interrupt mapping for PIT (timer).
    let mut timer_int_mapping = None;
    for entry in iter.clone() {
        if let MpTableEntry::IoInterruptAssignment(entry) = entry {
            if entry.source_bus_id == isa_bus.bus_id && entry.source_bus_irq == 0 {
                assert!(timer_int_mapping.is_none(), "multiple timer IRQs found");
                timer_int_mapping = Some(entry);
            }
        }
    }

    let timer_int_mapping = timer_int_mapping.expect("timer IRQ not found");
    assert_eq!(timer_int_mapping.dest_io_apic_id, ioapic.io_apic_id);
    info!("timer IRQ: {:?}", timer_int_mapping);

    io_apic::init(PAddr::new(ioapic.io_apic_address as usize));
    use_ioapic(|ioapic| {
        ioapic
            .enable_irq_at(timer_int_mapping.dest_io_apic_intin, TIMER_IRQ)
            .unwrap();
    });
}
