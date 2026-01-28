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

/// Loads the MP configuration table to locate CPUs and APICs.
pub fn init() {
    let mp_table = find_mpfp_table().expect("failed to locate MP floating pointer table");
    let mp_config_addr = mp_table.physical_addr_pointer as usize;
    let mp_config = unsafe { paddr2ptr::<MpConfigTable>(mp_config_addr) };
    let mut entry_addr = mp_config_addr + size_of::<MpConfigTable>();
    for _ in 0..mp_config.entry_count {
        let type_byte = unsafe { paddr2ptr::<u8>(entry_addr) };
        let entry_size = match *type_byte {
            0 => {
                let entry = unsafe { paddr2ptr::<ProcessorEntry>(entry_addr) };
                let id = entry.local_apic_id;
                println!("Processor: id={}", id);
                20
            }
            2 => {
                let entry = unsafe { paddr2ptr::<IoApicEntry>(entry_addr) };
                let id = entry.io_apic_id;
                let addr = entry.io_apic_address;
                println!("I/O APIC: id={}, address={:08x}", id, addr);
                8
            }
            _ => {
                // println!("unknown entry type: {}", type_byte);
                8
            }
        };

        entry_addr += entry_size;
    }
}
