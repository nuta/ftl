//! Sv48 page table.
use core::{
    borrow::{Borrow, BorrowMut},
    ops::DerefMut,
};

use bitfields::{bitfields, B10, B2, B44};

use crate::{
    address::{PAddr, UAddr},
    memory,
    memory_pool::{memory_pool_mut, paddr2frame, Frame},
    ref_count::{SharedRef, UniqueRef},
};

use super::PAGE_SIZE;

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
    /// Returns if the entry is leaf (huge page). Otherwise, if `false`,
    /// the entry points to the next level page table.
    pub fn is_leaf_entry(&self) -> bool {
        self.readable() || self.writable() || self.executable()
    }

    pub fn paddr(&self) -> PAddr {
        PAddr::new((self.ppn() as usize) << 12)
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

pub struct Page4K {
    pub bytes: [u8; 4096],
}

pub enum TableOrLeaf {
    Table(Pte),
    Leaf(Pte),
}

pub struct RawPageTable<const LEVEL: usize> {
    entries: [Pte; ENTRIES_PER_TABLE],
}

impl<const LEVEL: usize> RawPageTable<LEVEL> {
    pub const fn new() -> Self {
        // FIXME: Map kernel pages.
        Self {
            entries: [Pte::zeroed(); ENTRIES_PER_TABLE],
        }
    }

    fn lookup_unchecked(&self, index: usize) -> Option<TableOrLeaf> {
        debug_assert!(index < ENTRIES_PER_TABLE);

        let entry = self.entries[index];

        if !entry.valid() {
            return None;
        }

        if entry.is_leaf_entry() {
            Some(TableOrLeaf::Leaf(entry))
        } else {
            Some(TableOrLeaf::Table(entry))
        }
    }

    /// Maps the next level page table.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `table` is a valid page table of the next
    /// level.
    unsafe fn map_table_unchecked(&mut self, index: usize, table: PAddr) {
        debug_assert!(index < ENTRIES_PER_TABLE);

        // Check physical page number (PPN) is aligned and the reserved bits are
        // zero.
        let paddr = table.as_usize() as u64;
        debug_assert!(paddr & 0xffc00000000003ff == 0);

        let mut pte = Pte::zeroed();
        pte.set_ppn(paddr >> 10);
        pte.set_valid(true);

        self.entries[index] = pte;
    }
}

impl RawPageTable<3> {
    fn lookup(&self, uaddr: UAddr) -> Option<TableOrLeaf> {
        self.lookup_unchecked(uaddr.vpn3())
    }

    fn map_table(&mut self, uaddr: UAddr, table: SharedRef<RawPageTable<2>>) {
        unsafe {
            // SAFETY: We know that table is surely a table of level 3 thanks to
            //         the type system.
            self.map_table_unchecked(uaddr.vpn3(), SharedRef::paddr(&table));
            // SAFETY: We'll drop the reference count when unmapping the page table
            //         entry.
            SharedRef::leak(table);
        }
    }
}

impl<const LEVEL: usize> Drop for RawPageTable<LEVEL> {
    fn drop(&mut self) {
        for entry in self.entries {
            if entry.valid() {
                let paddr = PAddr::new((entry.ppn() << 10) as usize);

                // SAFETY: map_table() requires a corresponding SharedRef
                //         when mapping the entry. Also, we deliberately
                //         leaked the reference count then. Thus, we can
                //         safely reconstruct the SharedRef without
                //         updating the reference count.
                let table = unsafe {
                    if LEVEL > 0 && entry.is_leaf_entry() {
                        unimplemented!("huge page")
                    } else {
                        debug_assert!(LEVEL == 0 || entry.is_leaf_entry());

                        match paddr2frame(paddr) {
                            Some(frame) => match *frame {
                                Frame::PageTable(ref inner) => {
                                    SharedRef::new(inner)
                                }
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        }
                    }
                };

                // Drop the sub page table here. I know this line is not
                // necessary but just for the sake of clarity.
                drop(table);
            }
        }
    }
}

pub type PageTableL2 = RawPageTable<2>;
pub type PageTableL1 = RawPageTable<1>;
pub type PageTableL0 = RawPageTable<0>;

#[repr(transparent)]
pub struct PageTable(pub RawPageTable<3>);

impl PageTable {
    pub fn new() -> Self {
        Self(RawPageTable::new())
    }

    pub fn map_recursively(&mut self, uaddr: UAddr, page: SharedRef<Page4K>) {
        let l2table = match self.0.lookup(uaddr) {
            Some(TableOrLeaf::Table(pte)) => match paddr2frame(pte.paddr()) {
                Some(frame) => match *frame {
                    Frame::PageTableL2(ref inner) => SharedRef::new(inner),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            Some(TableOrLeaf::Leaf(_)) => unreachable!(),
            None => {
                let l2table = memory::allocate_and_initialize(
                    PAGE_SIZE,
                    |pool, vaddr| {
                        pool.initialize_page_table_l2(vaddr, PAGE_SIZE).unwrap()
                    },
                );

                self.0.map_table(uaddr, SharedRef::inc_ref(&l2table));
                l2table
            }
        };
    }
}
