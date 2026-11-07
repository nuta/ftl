//! MultiProcessor Specification Version 1.4.
//!
//! <https://web.archive.org/web/20121002210153/http://download.intel.com/design/archives/processors/pro/docs/24201606.pdf>

use core::ops::Range;

use crate::address::PAddr;

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
const ENTRY_TYPE_IO_APIC: u8 = 2;

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

unsafe fn paddr2ptr<T>(paddr: usize) -> &'static T {
    let vaddr = super::paddr2vaddr(PAddr::new(paddr));
    unsafe { &*(vaddr.as_usize() as *const T) }
}

fn find_mpfp_table() -> Option<&'static MpPointerTable> {
    // > This structure must be stored in at least one of the following memory
    // > locations, because the operating system searches for the MP floating
    // > pointer structure in the order described below:
    const SEARCH_LOCATIONS: [Range<usize>; 3] = [
        // > a. In the first kilobyte of Extended BIOS Data Area (EBDA), or
        0x80000..0x81000,
        // > b. Within the last kilobyte of system base memory (639K-640K).
        0x9fc00..0xa0000,
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
    IoApic(&'a IoApicEntry),
    Unknown,
}

#[derive(Clone)]
struct MpTableIter<'a> {
    mp_config: &'a MpConfigTable,
    index: u16,
    entry_addr: usize,
    table_end: usize,
}

impl<'a> MpTableIter<'a> {
    fn new(mp_table: &'a MpPointerTable) -> Self {
        let mp_config_addr = mp_table.physical_addr_pointer as usize;
        let mp_config = unsafe { paddr2ptr::<MpConfigTable>(mp_config_addr) };
        let entry_addr = mp_config_addr + size_of::<MpConfigTable>();
        let table_end = mp_config_addr + mp_config.length as usize;
        Self {
            mp_config,
            index: 0,
            entry_addr,
            table_end,
        }
    }
}

impl<'a> Iterator for MpTableIter<'a> {
    type Item = MpTableEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.mp_config.entry_count != 0 && self.index >= self.mp_config.entry_count {
            return None;
        }

        if self.entry_addr >= self.table_end {
            // qboot does not set mp_config.entry_count.
            return None;
        }

        let type_byte = unsafe { paddr2ptr::<u8>(self.entry_addr) };
        let (entry, entry_size) = match *type_byte {
            ENTRY_TYPE_PROCESSOR => {
                let entry = unsafe { paddr2ptr::<ProcessorEntry>(self.entry_addr) };
                (MpTableEntry::Processor(entry), 20)
            }
            ENTRY_TYPE_IO_APIC => {
                let entry = unsafe { paddr2ptr::<IoApicEntry>(self.entry_addr) };
                (MpTableEntry::IoApic(entry), 8)
            }
            _ => {
                // trace!("unknown entry type: {}", type_byte);
                (MpTableEntry::Unknown, 8)
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

    for entry in iter.clone() {
        if let MpTableEntry::Processor(entry) = entry {
            trace!("processor: {:x}", entry.local_apic_id);
        }
    }

    // Find the I/O APIC.
    let mut ioapic = None;
    for entry in iter.clone() {
        if let MpTableEntry::IoApic(entry) = entry {
            assert!(ioapic.is_none(), "multiple I/O APICs found");
            ioapic = Some(entry);
        }
    }

    let ioapic = ioapic.expect("I/O APIC not found");
    let ioapic_address = PAddr::new(ioapic.io_apic_address as usize);
    super::io_apic::init(ioapic_address);
}
