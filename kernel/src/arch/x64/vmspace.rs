use core::arch::asm;
use core::ops::Range;

use ftl_types::error::ErrorCode;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::is_aligned;
use ftl_utils::spinlock::SpinLock;

use crate::address::PAddr;
use crate::address::UAddr;
use crate::address::VAddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;

pub const MIN_PAGE_SIZE: usize = 4096;
pub const KERNEL_BASE: usize = 0xffff_8000_0000_0000;
pub const USER_ADDR_END: usize = 0x0000_8000_0000_0000;

const ENTRIES_PER_TABLE: usize = 512;
const GIGA_PAGE_SIZE: usize = 1024 * 1024 * 1024;
const DIRECT_MAP_SIZE: usize = 4 * GIGA_PAGE_SIZE;
pub const DIRECT_MAP_END: PAddr = PAddr::new(DIRECT_MAP_SIZE);

// Page table entry flags.
const PTE_V: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_U: u64 = 1 << 2;
const PTE_HUGE: u64 = 1 << 7;
const PTE_NX: u64 = 1 << 63;

/// The boot-time PML4. The boot code will populate this.
pub(super) static mut BOOT_PML4: Table = Table([Pte(0); ENTRIES_PER_TABLE]);

/// The boot-time PDPT.
pub(super) static BOOT_PDPT: Table = {
    let mut pdpt = Table([Pte(0); ENTRIES_PER_TABLE]);

    // Map the first 4GiB of physical memory. It should be plenty enough to
    // boot the kernel.
    let mut i = 0;
    while i < DIRECT_MAP_SIZE / GIGA_PAGE_SIZE {
        pdpt.0[i] = Pte::new(PAddr::new(i * GIGA_PAGE_SIZE), PTE_V | PTE_W | PTE_HUGE);
        i += 1;
    }

    pdpt
};

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

    const fn is_user(self) -> bool {
        self.0 & PTE_U != 0
    }

    const fn paddr(self) -> PAddr {
        let paddr = self.0 & 0x000f_ffff_ffff_f000;
        PAddr::new(paddr as usize)
    }
}

pub fn paddr2vaddr(paddr: PAddr) -> VAddr {
    VAddr::new(paddr.as_usize() | KERNEL_BASE)
}

pub fn vaddr2paddr(vaddr: VAddr) -> PAddr {
    PAddr::new(vaddr.as_usize() & !KERNEL_BASE)
}

fn paddr_to_table_mut(paddr: PAddr) -> &'static mut Table {
    let vaddr: VAddr = paddr2vaddr(paddr);
    unsafe { &mut *(vaddr.as_usize() as *mut Table) }
}

fn alloc_table() -> Result<PAddr, ErrorCode> {
    let paddr = PAGE_ALLOCATOR
        .alloc(MIN_PAGE_SIZE, PageType::Zeroed)
        .ok_or(ErrorCode::OutOfMemory)?;

    Ok(paddr)
}

fn ensure_next_table(table: &mut Table, index: usize) -> Result<&mut Table, ErrorCode> {
    let entry = &mut table.0[index];
    let next_table_paddr = if !entry.is_present() {
        let paddr = alloc_table()?;
        // User mappings require U/S at every page-table level.
        *entry = Pte::new(paddr, PTE_V | PTE_W | PTE_U);
        paddr
    } else {
        if entry.is_huge() {
            return Err(ErrorCode::Unsupported);
        }

        entry.paddr()
    };

    Ok(paddr_to_table_mut(next_table_paddr))
}

fn get_next_table(table: &mut Table, index: usize) -> Result<&mut Table, ErrorCode> {
    let entry = table.0[index];
    if !entry.is_present() {
        return Err(ErrorCode::NotFound);
    }
    if entry.is_huge() {
        return Err(ErrorCode::Unsupported);
    }

    Ok(paddr_to_table_mut(entry.paddr()))
}

fn unmap_page(pml4: &mut Table, uaddr: usize) -> Result<(), ErrorCode> {
    let pdpt = get_next_table(pml4, pml4_index(uaddr))?;
    let pdt = get_next_table(pdpt, pdpt_index(uaddr))?;
    let pt = get_next_table(pdt, pdt_index(uaddr))?;

    let entry = &mut pt.0[pt_index(uaddr)];
    if !entry.is_present() {
        return Err(ErrorCode::NotFound);
    }

    if entry.is_huge() {
        return Err(ErrorCode::Unsupported);
    }

    *entry = Pte(0);

    unsafe {
        asm!("invlpg [{}]", in(reg) uaddr, options(nostack, preserves_flags));
    }

    Ok(())
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

pub struct VmSpace {
    mutable: SpinLock<Mutable>,
    cr3: u64,
}

fn read_cr3() -> u64 {
    let cr3: u64;
    unsafe {
        asm!("mov {cr3}, cr3", cr3 = out(reg) cr3);
    }
    cr3
}

fn write_cr3(cr3: u64) {
    unsafe {
        asm!("mov cr3, {cr3}", cr3 = in(reg) cr3);
    }
}

impl Drop for VmSpace {
    fn drop(&mut self) {
        let current_cr3 = read_cr3();
        if current_cr3 == self.cr3 {
            // This CPU is still using this VM space. Switch to the boot time
            // one before freeing the space.
            let pml4_vaddr = VAddr::new(&raw const BOOT_PML4 as usize);
            let pml4_paddr = vaddr2paddr(pml4_vaddr);
            write_cr3(pml4_paddr.as_u64());
        }

        unsafe { free_table(PAddr::new(self.cr3 as usize), 4) };
    }
}

unsafe fn free_table(paddr: PAddr, level: usize) {
    // Free child tables first.
    if level > 1 {
        let table = paddr_to_table_mut(paddr);
        for pte in &table.0[..] {
            if pte.is_present() && pte.is_user() && !pte.is_huge() {
                unsafe { free_table(pte.paddr(), level - 1) };
            }
        }
    }

    // Free this page.
    unsafe { PAGE_ALLOCATOR.free(paddr, MIN_PAGE_SIZE) };
}

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        let pdpt_vaddr = VAddr::new(BOOT_PDPT.0.as_ptr() as usize);
        let pdpt_paddr = vaddr2paddr(pdpt_vaddr);
        let pml4_paddr = PAGE_ALLOCATOR
            .alloc(4096, PageType::Zeroed)
            .ok_or(ErrorCode::OutOfMemory)?;
        let pml4_vaddr = paddr2vaddr(pml4_paddr);
        let pml4 = unsafe { &mut *(pml4_vaddr.as_usize() as *mut Table) };

        // Map KERNEL_BASE to BOOT_PDPT.
        pml4.0[256] = Pte::new(pdpt_paddr, PTE_V | PTE_W);

        Ok(Self {
            cr3: pml4_paddr.as_u64(),
            mutable: SpinLock::new(Mutable { pml4: pml4_vaddr }),
        })
    }

    pub fn switch(&self) {
        if read_cr3() == self.cr3 {
            return;
        }

        write_cr3(self.cr3);
    }

    pub fn map(
        &self,
        uaddr: UAddr,
        paddr: PAddr,
        len: usize,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        let uaddr = uaddr.as_usize();

        // Validate the page attributes.
        let allowed_attrs = PageAttrs::READ | PageAttrs::WRITE | PageAttrs::EXEC;
        if !allowed_attrs.contains(attrs) {
            return Err(ErrorCode::InvalidPageAttrs);
        }

        if !is_aligned(uaddr, MIN_PAGE_SIZE)
            || !paddr.is_aligned(MIN_PAGE_SIZE)
            || !is_aligned(len, MIN_PAGE_SIZE)
        {
            return Err(ErrorCode::NotAligned);
        }

        let mutable = self.mutable.lock();
        let pml4 = unsafe { &mut *(mutable.pml4.as_usize() as *mut Table) };
        let pdpt = ensure_next_table(pml4, pml4_index(uaddr))?;
        let pdt = ensure_next_table(pdpt, pdpt_index(uaddr))?;
        let pt = ensure_next_table(pdt, pdt_index(uaddr))?;
        let entry = &mut pt.0[pt_index(uaddr)];

        if entry.is_present() {
            return Err(ErrorCode::AlreadyMapped);
        }

        // Translate the page attributes into page table entry flags.
        let mut flags = PTE_V | PTE_U;
        if attrs.contains(PageAttrs::WRITE) {
            flags |= PTE_W;
        }
        if !attrs.contains(PageAttrs::EXEC) {
            flags |= PTE_NX;
        }

        *entry = Pte::new(paddr, flags);

        // Invalidate the page in the TLB to let CPU reread the new entry.
        unsafe {
            asm!("invlpg [{}]", in(reg) uaddr, options(nostack, preserves_flags));
        }

        Ok(())
    }

    pub fn unmap(&self, uaddr: UAddr, len: usize) -> Result<(), ErrorCode> {
        let start = uaddr.as_usize();
        if !is_aligned(start, MIN_PAGE_SIZE) || !is_aligned(len, MIN_PAGE_SIZE) {
            return Err(ErrorCode::NotAligned);
        }

        let end = start.checked_add(len).ok_or(ErrorCode::OutOfBounds)?;
        let mutable = self.mutable.lock();
        let pml4 = unsafe { &mut *(mutable.pml4.as_usize() as *mut Table) };

        let mut addr = start;
        while addr < end {
            match unmap_page(pml4, addr) {
                // Ignore missing pages. It happens only if the VMO exist due
                // to lazy mapping.
                Ok(()) | Err(ErrorCode::NotFound) => {}
                Err(e) => return Err(e),
            }
            addr += MIN_PAGE_SIZE;
        }

        Ok(())
    }
}

unsafe extern "C" {
    static __kernel_memory: u8;
    static __kernel_memory_end: u8;
}

pub(super) fn get_kernel_reserved_range() -> Range<PAddr> {
    let start = VAddr::new(&raw const __kernel_memory as usize);
    let end = VAddr::new(&raw const __kernel_memory_end as usize);
    let start_paddr = vaddr2paddr(start);
    let end_paddr = vaddr2paddr(end);
    start_paddr..end_paddr
}
