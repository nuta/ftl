use core::arch;
use core::mem;
use core::num::NonZeroUsize;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_utils::alignment::is_aligned;

use crate::arch::paddr2vaddr;
use crate::arch::PAGE_SIZE;
use crate::folio::Folio;
use crate::spinlock::SpinLock;

const ENTRIES_PER_TABLE: usize = 512;
const ENTRY_AF: u64 = 1 << 10;
const ENTRY_AP_READWRITE_USER: u64 = 0b01 << 6;
const ENTRY_TYPE_TABLE: u64 = 0b11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Entry(u64);

impl Entry {
    pub fn table_entry(paddr: PAddr, flags: u64) -> Self {
        assert!(is_aligned(paddr.as_usize(), PAGE_SIZE));
        Self(paddr.as_usize() as u64 | flags | ENTRY_AF | ENTRY_TYPE_TABLE)
    }

    pub fn is_invalid(&self) -> bool {
        self.0 & 1 == 0
    }

    pub fn paddr(&self) -> PAddr {
        let raw = self.0 & 0x000_ffff_ffff_f000;

        // Entry::table_entry() guarantees we never map to 0.
        let nonzero = unsafe { NonZeroUsize::new_unchecked(raw as usize) };

        PAddr::from_nonzero(nonzero)
    }
}

#[repr(transparent)]
struct Table([Entry; ENTRIES_PER_TABLE]);

impl Table {
    pub fn get_mut_by_vaddr(&mut self, vaddr: VAddr, level: usize) -> &mut Entry {
        let index = (vaddr.as_usize() >> (12 + 9 * level)) & 0x1ff;
        &mut self.0[index]
    }
}

struct PageTable {
    l0_table: Folio,
}

impl PageTable {
    pub fn new() -> Result<PageTable, FtlError> {
        let l0_table = Folio::alloc(size_of::<Table>())?;
        Ok(PageTable { l0_table })
    }

    pub fn map_kernel_space(&mut self) -> Result<(), FtlError> {
        self.map_range(VAddr::new(0x0800_0000).unwrap(), PAddr::new(0x0800_0000).unwrap(), 0x2000)?;
        self.map_range(VAddr::new(0x0900_0000).unwrap(), PAddr::new(0x0900_0000).unwrap(), 0x1000)?;
        self.map_range(VAddr::new(0x4010_0000).unwrap(), PAddr::new(0x4010_0000).unwrap(), 0x1200000)?;
        Ok(())
    }

    pub fn map_range(&mut self, vaddr: VAddr, paddr: PAddr, len: usize) -> Result<(), FtlError> {
        assert!(is_aligned(len, PAGE_SIZE));

        for offset in (0..len).step_by(PAGE_SIZE) {
            self.map(vaddr.add(offset), paddr.add(offset), PAGE_SIZE)?;
        }
        Ok(())
    }

    pub fn map(&mut self, vaddr: VAddr, paddr: PAddr, len: usize) -> Result<(), FtlError> {
        // trace!("map: {:08x} -> {:08x}", vaddr.as_usize(), paddr.as_usize());
        assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        assert!(is_aligned(paddr.as_usize(), PAGE_SIZE));
        assert!(is_aligned(len, PAGE_SIZE));

        // We only support mapping a single 4KB page for now.
        assert_eq!(len, PAGE_SIZE);
        self.map_4kb(vaddr, paddr)?;

        // FIXME:
        unsafe {
            arch::asm!("dsb ish");
            arch::asm!("isb");
            arch::asm!("tlbi vmalle1is");
            arch::asm!("dsb ish");
            arch::asm!("isb");
        }

        Ok(())
    }

    fn paddr2table(&mut self, paddr: PAddr) -> Result<&mut Table, FtlError> {
        let vaddr = paddr2vaddr(paddr)?;
        Ok(unsafe { &mut *vaddr.as_mut_ptr() })
    }

    fn map_4kb(&mut self, vaddr: VAddr, paddr: PAddr) -> Result<(), FtlError> {
        assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        assert!(is_aligned(paddr.as_usize(), PAGE_SIZE));

        println!("map_4kb: {:08x} -> {:08x}", vaddr.as_usize(), paddr.as_usize());
        let mut table = self.paddr2table(self.l0_table.paddr())?;
        for level in (1..=3).rev() {
            let entry = table.get_mut_by_vaddr(vaddr, level);
            if entry.is_invalid() {
                // Allocate a new table.
                let new_table = Folio::alloc(size_of::<Table>())?;
                *entry = Entry::table_entry(new_table.paddr(), 0);

                // TODO: Initialize the new table with zeros.

                // This vmspace object owns the allocated folio.
                // TODO: deallocate on Drop
                mem::forget(new_table);
            }

            // Traverse to the next table.
            let next_table_paddr = entry.paddr();
            table = self.paddr2table(next_table_paddr)?;
        }

        let entry = table.get_mut_by_vaddr(vaddr, 0);
        if !entry.is_invalid() {
            return Err(FtlError::AlreadyMapped);
        }

        *entry = Entry::table_entry(paddr, ENTRY_AP_READWRITE_USER);
        Ok(())
    }
}

pub struct VmSpace {
    table: SpinLock<PageTable>,
    pub(super) ttbr0: u64,
}

impl VmSpace {
    pub fn new() -> Result<VmSpace, FtlError> {
        let mut table = PageTable::new()?;
        table.map_kernel_space()?;

        let ttbr0 = table.l0_table.paddr().as_usize() as u64;
        Ok(VmSpace {
            ttbr0,
            table: SpinLock::new(table),
        })
    }

    pub fn map(& self, vaddr: VAddr, paddr: PAddr, len: usize) -> Result<(), FtlError> {
        self.table.lock().map(vaddr, paddr, len)
    }
}