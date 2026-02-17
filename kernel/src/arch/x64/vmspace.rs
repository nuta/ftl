use core::arch::asm;

use ftl_types::error::ErrorCode;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::is_aligned;

use crate::address::PAddr;
use crate::address::VAddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::spinlock::SpinLock;

pub const MIN_PAGE_SIZE: usize = 4096;
pub(super) const KERNEL_BASE: usize = 0xffff_8000_0000_0000;

const ENTRIES_PER_TABLE: usize = 512;
const GIGA_PAGE_SIZE: usize = 1024 * 1024 * 1024;

// Page table entry flags.
const PTE_V: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7;

/// A page table, at any level (PML4, PDPT, PDT, PT).
#[repr(align(4096))]
pub(super) struct Table([Pte; ENTRIES_PER_TABLE]);

/// A page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct Pte(u64);

impl Pte {
    const fn new(paddr: PAddr, flags: u64) -> Self {
        debug_assert!(paddr.is_aligned(4096));

        Self(paddr.as_u64() | flags)
    }

    const fn is_present(self) -> bool {
        self.0 & PTE_V != 0
    }

    const fn is_huge(self) -> bool {
        self.0 & PTE_HUGE != 0
    }

    const fn paddr(self) -> PAddr {
        let paddr = self.0 & 0x000f_ffff_ffff_f000;
        PAddr::new(paddr as usize)
    }
}

/// The boot-time PDPT.
pub(super) static BOOT_PDPT: Table = {
    let mut pdpt = Table([Pte(0); ENTRIES_PER_TABLE]);

    // Map the first 4GiB of physical memory. It should be plenty enough to
    // boot the kernel.
    let mut i = 0;
    while i < 4 {
        pdpt.0[i] = Pte::new(PAddr::new(i * GIGA_PAGE_SIZE), PTE_V | PTE_W | PTE_HUGE);
        i += 1;
    }

    pdpt
};

/// The boot-time PML4. The boot code will populate this.
pub(super) static mut BOOT_PML4: Table = Table([Pte(0); ENTRIES_PER_TABLE]);

pub fn paddr2vaddr(paddr: PAddr) -> VAddr {
    debug_assert!(
        paddr.as_usize() < KERNEL_BASE,
        "{paddr} is suspiciously high for a physical address"
    );

    VAddr::new(paddr.as_usize() + KERNEL_BASE)
}

pub fn vaddr2paddr(vaddr: VAddr) -> PAddr {
    debug_assert!(
        vaddr.as_usize() >= KERNEL_BASE,
        "{vaddr} is not mapped in the kernel"
    );

    PAddr::new(vaddr.as_usize() - KERNEL_BASE)
}

fn paddr_to_table_mut(paddr: PAddr) -> &'static mut Table {
    let vaddr: VAddr = paddr2vaddr(paddr);
    unsafe { &mut *(vaddr.as_usize() as *mut Table) }
}

fn alloc_table() -> Result<PAddr, ErrorCode> {
    let paddr = PAGE_ALLOCATOR
        .alloc(MIN_PAGE_SIZE)
        .ok_or(ErrorCode::OutOfMemory)?;

    // FIXME: Guarantee that the page allocator zeros pages.
    paddr_to_table_mut(paddr).0.fill(Pte(0));

    Ok(paddr)
}

fn ensure_next_table(table: &mut Table, index: usize) -> Result<&mut Table, ErrorCode> {
    let entry = &mut table.0[index];
    let next_table_paddr = if !entry.is_present() {
        let paddr = alloc_table()?;
        *entry = Pte::new(paddr, PTE_V | PTE_W);
        paddr
    } else {
        if entry.is_huge() {
            return Err(ErrorCode::Unsupported);
        }

        entry.paddr()
    };

    Ok(paddr_to_table_mut(next_table_paddr))
}

const fn pml4_index(vaddr: usize) -> usize {
    (vaddr >> 39) & 0x1ff
}

const fn pdpt_index(vaddr: usize) -> usize {
    (vaddr >> 30) & 0x1ff
}

const fn pdt_index(vaddr: usize) -> usize {
    (vaddr >> 21) & 0x1ff
}

const fn pt_index(vaddr: usize) -> usize {
    (vaddr >> 12) & 0x1ff
}

struct Mutable {
    pml4: VAddr,
}

#[repr(align(4096))]
pub struct VmSpace {
    mutable: SpinLock<Mutable>,
    cr3: u64,
}

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        let pdpt_vaddr = VAddr::new(BOOT_PDPT.0.as_ptr() as usize);
        let pdpt_paddr = vaddr2paddr(pdpt_vaddr);
        let pml4_paddr = PAGE_ALLOCATOR.alloc(4096).ok_or(ErrorCode::OutOfMemory)?;
        let pml4_vaddr = paddr2vaddr(pml4_paddr);
        let pml4 = unsafe { &mut *(pml4_vaddr.as_usize() as *mut Table) };

        // FIXME: Guarantee that the page allocator zeros pages.
        pml4.0.fill(Pte(0));

        // Map KERNEL_BASE to BOOT_PDPT.
        pml4.0[256] = Pte::new(pdpt_paddr, PTE_V);

        Ok(Self {
            cr3: pml4_paddr.as_u64(),
            mutable: SpinLock::new(Mutable { pml4: pml4_vaddr }),
        })
    }

    pub fn switch(&self) {
        unsafe {
            asm!("mov cr3, {cr3}", cr3 = in(reg) self.cr3);
        }
    }

    pub fn map(
        &self,
        uaddr: usize,
        paddr: PAddr,
        len: usize,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        if !is_aligned(uaddr, MIN_PAGE_SIZE)
            || !paddr.is_aligned(MIN_PAGE_SIZE)
            || !is_aligned(len, MIN_PAGE_SIZE)
        {
            return Err(ErrorCode::InvalidArgument);
        }

        // Keep kernel mappings immutable from this API.
        if uaddr >= KERNEL_BASE {
            return Err(ErrorCode::NotAllowed);
        }

        let mutable = self.mutable.lock();
        let pml4 = unsafe { &mut *(mutable.pml4.as_usize() as *mut Table) };
        let pdpt = ensure_next_table(pml4, pml4_index(uaddr))?;
        let pdt = ensure_next_table(pdpt, pdpt_index(uaddr))?;
        let pt = ensure_next_table(pdt, pdt_index(uaddr))?;
        let entry = &mut pt.0[pt_index(uaddr)];

        if entry.is_present() {
            return Err(ErrorCode::AlreadyExists);
        }

        *entry = Pte::new(paddr, PTE_V | attrs.as_usize() as u64);
        Ok(())
    }
}
