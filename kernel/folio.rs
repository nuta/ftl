use core::alloc::Layout;

use ftl_types::address::PAddr;
use ftl_types::error::FtlError;

use crate::arch;
use crate::handle::Handleable;
use crate::memory::GLOBAL_ALLOCATOR;
use crate::vm::KVAddr;
use crate::vm::KVAddrArchExt;

/// Folio, a contigous physical memory "pages".
pub struct Folio {
    /// The base address of the folio.
    paddr: PAddr,
    /// The virtual address of the folio if it's always mapped to the kernel's
    /// address space.
    kvaddr: Option<KVAddr>,
    /// The size of the folio in bytes.
    size: usize,
}

impl Folio {
    pub fn alloc(size: usize) -> Result<Folio, FtlError> {
        let layout = match Layout::from_size_align(size, arch::PAGE_SIZE) {
            Ok(layout) => layout,
            Err(_) => return Err(FtlError::InvalidParams),
        };

        if layout.size() == 0 {
            return Err(FtlError::InvalidParams);
        }

        if layout.size() % arch::PAGE_SIZE != 0 {
            return Err(FtlError::InvalidParams);
        }

        let kvaddr = GLOBAL_ALLOCATOR.alloc_as_kvaddr(layout)?;

        Ok(Folio {
            paddr: kvaddr.paddr(),
            kvaddr: Some(kvaddr),
            size: layout.size(),
        })
    }

    /// Returns the physical address of the folio.
    pub fn paddr(&self) -> PAddr {
        self.paddr
    }

    /// Returns the size of the folio in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Handleable for Folio {}
