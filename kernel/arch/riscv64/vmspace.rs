use core::arch::asm;
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
const PPN_SHIFT: usize = 12;

const PTE_V: u64 = 1 << 0;
const PTE_R: u64 = 1 << 1;
const PTE_W: u64 = 1 << 2;
const PTE_X: u64 = 1 << 3;
const PTE_PPN_SHIFT: usize = 10;

const SATP_MODE_SV48: u64 = 9 << 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Entry(u64);

impl Entry {
    pub fn new(paddr: PAddr, flags: u64) -> Self {
        assert!(is_aligned(paddr.as_usize(), PAGE_SIZE));

        let ppn = paddr.as_usize() as u64 >> PPN_SHIFT;
        Self(ppn << PTE_PPN_SHIFT | flags)
    }

    pub fn is_invalid(&self) -> bool {
        self.0 & PTE_V == 0
    }

    pub fn ppn(&self) -> u64 {
        self.0 >> PTE_PPN_SHIFT
    }

    pub fn paddr(&self) -> PAddr {
        let raw = self.ppn() << PPN_SHIFT;
        // FIXME: this is actually unsafe
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
        self.map_range(
            VAddr::new(0x8020_0000).unwrap(),
            PAddr::new(0x8020_0000).unwrap(),
            0x8ff00000 - 0x8020_0000,
        )?;
        self.map_range(
            VAddr::new(0xc000000).unwrap(),
            PAddr::new(0xc000000).unwrap(),
            0x400000,
        )?;
        // UART
        self.map_range(
            VAddr::new(0x1000_0000).unwrap(),
            PAddr::new(0x1000_0000).unwrap(),
            0x1000,
        )?;
        // Virtio
        self.map_range(
            VAddr::new(0x10001000).unwrap(),
            PAddr::new(0x10001000).unwrap(),
            0x1000,
        )?;
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

        for offset in (0..len).step_by(PAGE_SIZE) {
            self.map_4kb(vaddr.add(offset), paddr.add(offset))?;
        }

        // FIXME: Invalidate TLB

        Ok(())
    }

    fn paddr2table(&mut self, paddr: PAddr) -> Result<&mut Table, FtlError> {
        let vaddr = paddr2vaddr(paddr)?;
        Ok(unsafe { &mut *vaddr.as_mut_ptr() })
    }

    fn map_4kb(&mut self, vaddr: VAddr, paddr: PAddr) -> Result<(), FtlError> {
        assert!(is_aligned(vaddr.as_usize(), PAGE_SIZE));
        assert!(is_aligned(paddr.as_usize(), PAGE_SIZE));

        // println!(
        //     "map_4kb: {:08x} -> {:08x}",
        //     vaddr.as_usize(),
        //     paddr.as_usize()
        // );
        let mut table = self.paddr2table(self.l0_table.paddr())?;
        for level in (1..=3).rev() {
            let entry = table.get_mut_by_vaddr(vaddr, level);
            if entry.is_invalid() {
                // Allocate a new table.
                let new_table = Folio::alloc(size_of::<Table>())?;
                *entry = Entry::new(new_table.paddr(), PTE_V);

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

        *entry = Entry::new(paddr, PTE_X | PTE_W | PTE_R | PTE_V);
        Ok(())
    }
}

struct Mutable {
    table: PageTable,
    next_free_vaddr: VAddr,
}

pub struct VmSpace {
    mutable: SpinLock<Mutable>,
    satp: u64,
}

impl VmSpace {
    pub fn new() -> Result<VmSpace, FtlError> {
        let mut table = PageTable::new()?;
        table.map_kernel_space()?;

        let table_paddr = table.l0_table.paddr().as_usize() as u64;
        let satp = SATP_MODE_SV48 | (table_paddr >> PPN_SHIFT);
        Ok(VmSpace {
            satp,
            mutable: SpinLock::new(Mutable {
                table,
                next_free_vaddr: VAddr::new(0x4000_0000).unwrap(), // FIXME:
            }),
        })
    }

    pub fn map_fixed(&self, vaddr: VAddr, paddr: PAddr, len: usize) -> Result<(), FtlError> {
        self.mutable.lock().table.map(vaddr, paddr, len)
    }

    pub fn map_anywhere(&self, paddr: PAddr, len: usize) -> Result<VAddr, FtlError> {
        assert!(is_aligned(len, PAGE_SIZE));

        let mut mutable = self.mutable.lock();
        let vaddr = mutable.next_free_vaddr;
        mutable.next_free_vaddr = mutable.next_free_vaddr.add(len);

        mutable.table.map(vaddr, paddr, len)?;
        Ok(vaddr)
    }

    pub fn switch(&self) {
        unsafe {
            // Do sfeence.vma before and even before switching the page
            // table to ensure all changes prior to this switch are visible.
            //
            // (The RISC-V Instruction Set Manual Volume II, Version 1.10, p. 58)
            asm!("
                sfence.vma
                csrw satp, {}
                sfence.vma
            ", in(reg) self.satp);
        }
    }
}