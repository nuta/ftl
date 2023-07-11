use bitfields::{bitfields, bool, B10, B2, B44};

use crate::{address::UAddr, ref_count::SharedRef};

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
    valid: bool,
    /// Readable bit.
    readable: bool,
    /// Writable bit.
    writable: bool,
    /// Executable bit.
    executable: bool,
    /// User bit. If set, user mode can access this page.
    user: bool,
    /// Global bit. If set, TLB entry is not invalidated on address space
    /// switch.
    global: bool,
    /// Accessed bit.
    accessed: bool,
    /// Dirty bit.
    dirty: bool,
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

impl UAddr {
    pub fn vpn3(&self) -> usize {
        (self.as_usize() >> 39) & 0x1ff
    }

    pub fn vpn2(&self) -> usize {
        (self.as_usize() >> 30) & 0x1ff
    }

    pub fn vpn1(&self) -> usize {
        (self.as_usize() >> 21) & 0x1ff
    }

    pub fn vpn0(&self) -> usize {
        (self.as_usize() >> 12) & 0x1ff
    }
}

/// Page table.
pub struct PageTable {
    entries: [Pte; ENTRIES_PER_TABLE],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [Pte::invalid(); ENTRIES_PER_TABLE],
        }
    }

    pub fn map_table(
        &mut self,
        uaddr: UAddr,
        table: SharedRef<SubPageTableL1>,
    ) {
        let paddr = SharedRef::paddr(&table).as_usize() as u64;

        // Check physical page number (PPN) is aligned and the reserved bits are
        // zero.
        debug_assert!(paddr & 0xffc00000000003ff == 0);

        let mut pte = Pte::zeroed();
        pte.set_ppn(paddr >> 10);
        pte.set_valid(true);

        self.entries[uaddr.vpn3()] = pte;

        // Safety: We'll drop the reference count when unmapping the page table
        //         entry.
        unsafe {
            SharedRef::leak(table);
        }
    }
}

pub struct SubPageTableL1 {
    pub entries: [Pte; ENTRIES_PER_TABLE],
}
