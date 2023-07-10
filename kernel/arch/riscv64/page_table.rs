use core::ptr::NonNull;

use bitfields::{bitfields, B1, B10, B2, B44};

use crate::{ref_count::UniqueRef, address::UAddr};

/// The number of entries in a page table in any level.
const ENTRIES_PER_TABLE: usize = 512;

/// Page table entry.
///
/// # Leaf/non-leaf PTE
///
/// If all three of `readable`, `writable`, and `executable` are zero,
/// the entry points to the next level page table (non-leaf PTE).
///
/// Otherwise, the entry points to a data page (leaf PTE), aka *huge page*.
#[bitfields(u64)]
#[derive(Copy, Clone)]
struct Pte {
    /// Valid bit. If not set, page fault occurs.
    valid: B1,
    /// Readable bit.
    readable: B1,
    /// Writable bit.
    writable: B1,
    /// Executable bit.
    executable: B1,
    /// User bit. If set, user mode can access this page.
    user: B1,
    /// Global bit. If set, TLB entry is not invalidated on address space
    /// switch.
    global: B1,
    /// Accessed bit.
    accessed: B1,
    /// Dirty bit.
    dirty: B1,
    /// Available for software (i.e. us!) to use freely.
    rsw: B2,
    /// Physical page number of the next level page table or the data page.
    ppn: B44,
    /// Reserved by hardware.
    reserved: B10,
}

impl Pte {
    pub const fn invalid() -> Pte {
        Pte::from_raw(0)
    }
}

/// Page table.
pub struct PageTable {
    pub entries: [Pte; ENTRIES_PER_TABLE],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [Pte::invalid(); ENTRIES_PER_TABLE],
        }
    }

    // pub fn map_table(&mut self, uaddr: UAddr, table: &SubPageTableL1) {
    //     // TODO:
    // }
}


pub struct SubPageTableL1 {
    pub entries: [Pte; ENTRIES_PER_TABLE],
}
