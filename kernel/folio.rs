use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;

use crate::arch;
use crate::memory::alloc_pages;

/// Memory pages.
pub struct Folio {
    vaddr: VAddr,
    paddr: PAddr,
    _len: usize,
}

impl Folio {
    pub fn map_paddr(paddr: PAddr, len: usize) -> Result<Folio, FtlError> {
        Ok(Folio {
            vaddr: arch::paddr2vaddr(paddr).ok_or(FtlError::InvalidAddress)?,
            paddr,
            _len: len,
        })
    }

    pub fn alloc(len: usize) -> Result<Folio, FtlError> {
        let num_pages = (len + 4095) / 4096; // FIXME:
        let vaddr = alloc_pages(num_pages).ok_or(FtlError::OutOfMemory)?;

        Ok(Folio {
            // FIXME:
            vaddr: VAddr::new(vaddr).unwrap(),
            paddr: PAddr::new(vaddr).unwrap(),
            _len: len,
        })
    }

    pub fn vaddr(&self) -> VAddr {
        self.vaddr
    }

    pub fn paddr(&self) -> PAddr {
        self.paddr
    }
}
