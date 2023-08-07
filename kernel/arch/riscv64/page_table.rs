//! Sv48 page table.
use core::{
    borrow::{Borrow, BorrowMut},
    ops::DerefMut,
};

use bitfields::{bitfields, B10, B2, B44};

use crate::{
    address::{PAddr, UAddr},
    memory,
    memory_pool::{paddr2frame, Frame},
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
        PAddr::new((self.ppn() as usize) << 10)
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

impl Page4K {
    pub fn zeroed() -> Self {
        Self { bytes: [0; 4096] }
    }

    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) {
        debug_assert!(offset + bytes.len() <= 4096);
        self.bytes[offset..offset + bytes.len()].copy_from_slice(bytes);
    }
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

    /// Maps a next level page table.
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

    /// Maps a leaf entry.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `frame` is a valid page table of the next
    /// level.
    unsafe fn map_leaf_unchecked(
        &mut self,
        index: usize,
        frame: PAddr,
        readable: bool,
        writable: bool,
        executable: bool,
        user: bool,
    ) {
        debug_assert!(index < ENTRIES_PER_TABLE);

        // Check physical page number (PPN) is aligned and the reserved bits are
        // zero.
        let paddr = frame.as_usize() as u64;
        debug_assert!(paddr & 0xffc00000000003ff == 0);

        let mut pte = Pte::zeroed();
        pte.set_ppn(paddr >> 10);
        pte.set_valid(true);
        pte.set_readable(readable);
        pte.set_writable(writable);
        pte.set_executable(executable);
        pte.set_user(user);

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

impl RawPageTable<2> {
    fn lookup(&self, uaddr: UAddr) -> Option<TableOrLeaf> {
        self.lookup_unchecked(uaddr.vpn2())
    }

    fn map_table(&mut self, uaddr: UAddr, table: SharedRef<RawPageTable<1>>) {
        unsafe {
            // SAFETY: We know that table is surely a table of level 3 thanks to
            //         the type system.
            self.map_table_unchecked(uaddr.vpn2(), SharedRef::paddr(&table));
            // SAFETY: We'll drop the reference count when unmapping the page table
            //         entry.
            SharedRef::leak(table);
        }
    }
}

impl RawPageTable<1> {
    fn lookup(&self, uaddr: UAddr) -> Option<TableOrLeaf> {
        self.lookup_unchecked(uaddr.vpn1())
    }

    fn map_table(&mut self, uaddr: UAddr, table: SharedRef<RawPageTable<0>>) {
        unsafe {
            // SAFETY: We know that table is surely a table of level 3 thanks to
            //         the type system.
            self.map_table_unchecked(uaddr.vpn1(), SharedRef::paddr(&table));
            // SAFETY: We'll drop the reference count when unmapping the page table
            //         entry.
            SharedRef::leak(table);
        }
    }
}

impl RawPageTable<0> {
    fn map_page4k(
        &mut self,
        uaddr: UAddr,
        page: SharedRef<Page4K>,
        readable: bool,
        writable: bool,
        executable: bool,
        user: bool,
    ) {
        unsafe {
            // SAFETY: We know that table is surely a table of level 3 thanks to
            //         the type system.
            self.map_leaf_unchecked(
                uaddr.vpn1(),
                SharedRef::paddr(&page),
                readable,
                writable,
                executable,
                user,
            );
            // SAFETY: We'll drop the reference count when unmapping the page table
            //         entry.
            SharedRef::leak(page);
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
    pub fn new() -> PageTable {
        PageTable(RawPageTable::new())
    }

    pub fn map_kernel_pages(&mut self){
        let l2table =
            memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
                pool.initialize_page_table_l2(vaddr, PAGE_SIZE).unwrap()
            });

        {
            let mut offset = 0;
            const GB: usize = 1024 * 1024 * 1024;
            let mut l2table = l2table.borrow_mut();
            while offset < 1 * GB {
                let uaddr = UAddr::new(0x00000000_80000000 + offset);
                let paddr = PAddr::new(0x00000000_80000000 + offset);

                let mut pte = Pte::zeroed();
                pte.set_valid(true);
                pte.set_readable(true);
                pte.set_writable(true);
                pte.set_executable(true);
                pte.set_user(false);
                pte.set_global(true);
                pte.set_ppn(paddr.as_usize() as u64 >> 10);
                l2table.entries[uaddr.vpn2()] = pte;

                offset += 1 * GB;
            }
        }

        self.0.map_table(UAddr::new(0x00000000_80000000), SharedRef::inc_ref(&l2table));
    }

    pub fn map_recursively(
        &mut self,
        uaddr: UAddr,
        page: SharedRef<Page4K>,
        readable: bool,
        writable: bool,
        executable: bool,
        user: bool,
    ) {
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

        let pte2 = l2table.borrow_mut().lookup(uaddr);
        let l1table = match pte2 {
            Some(TableOrLeaf::Table(pte)) => match paddr2frame(pte.paddr()) {
                Some(frame) => match *frame {
                    Frame::PageTableL1(ref inner) => SharedRef::new(inner),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            Some(TableOrLeaf::Leaf(_)) => unreachable!(),
            None => {
                let l1table = memory::allocate_and_initialize(
                    PAGE_SIZE,
                    |pool, vaddr| {
                        pool.initialize_page_table_l1(vaddr, PAGE_SIZE).unwrap()
                    },
                );

                l2table
                    .borrow_mut()
                    .map_table(uaddr, SharedRef::inc_ref(&l1table));
                l1table
            }
        };

        let pte1 = l1table.borrow_mut().lookup(uaddr);
        let l0table = match pte1 {
            Some(TableOrLeaf::Table(pte)) => match paddr2frame(pte.paddr()) {
                Some(frame) => match *frame {
                    Frame::PageTableL0(ref inner) => SharedRef::new(inner),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            Some(TableOrLeaf::Leaf(_)) => unreachable!(),
            None => {
                let l0table = memory::allocate_and_initialize(
                    PAGE_SIZE,
                    |pool, vaddr| {
                        pool.initialize_page_table_l0(vaddr, PAGE_SIZE).unwrap()
                    },
                );

                l1table
                    .borrow_mut()
                    .map_table(uaddr, SharedRef::inc_ref(&l0table));
                l0table
            }
        };

        l0table
            .borrow_mut()
            .map_page4k(uaddr, page, readable, writable, executable, user);
    }
}
