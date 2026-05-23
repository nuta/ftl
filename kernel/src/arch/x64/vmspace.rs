use crate::address::PAddr;
use crate::address::VAddr;

pub const KERNEL_BASE: usize = 0xffff_8000_0000_0000;

const ENTRIES_PER_TABLE: usize = 512;
const GIGA_PAGE_SIZE: usize = 1024 * 1024 * 1024;

// Page table entry flags.
const PTE_V: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7;

/// The boot-time PML4. The boot code will populate this.
pub(super) static mut BOOT_PML4: Table = Table([Pte(0); ENTRIES_PER_TABLE]);

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
}

pub fn paddr2vaddr(paddr: PAddr) -> VAddr {
    VAddr::new(paddr.as_usize() | KERNEL_BASE)
}
