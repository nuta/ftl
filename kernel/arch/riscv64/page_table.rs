use bitfields::{bitfields, B1, B44, B2, B10};

use crate::object::{KernelObject, ObjectKind};

const ENTRIES_PER_TABLE: usize = 512;

/// Page table entry.
#[bitfields(u64)]
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

pub struct PageTable {
    pub entries: [Pte; ENTRIES_PER_TABLE],
}

impl KernelObject for PageTable {
    fn kind(&self) -> ObjectKind {
        ObjectKind::PageTable
    }
}
