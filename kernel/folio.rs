use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;

use crate::arch::vaddr2paddr;
use crate::memory::AllocPagesError;
use crate::memory::AllocatedPages;

pub struct Folio {
    paddr: PAddr,
    len: usize,
    pages: Option<AllocatedPages>,
}

impl Folio {
    pub fn alloc(len: usize) -> Result<Folio, AllocPagesError> {
        let pages = AllocatedPages::alloc(len)?;
        Ok(Self {
            paddr: vaddr2paddr(VAddr::new(pages.as_ptr() as usize).unwrap()).unwrap(),
            len,
            pages: Some(pages),
        })
    }

    pub fn from_allocated_pages(pages: AllocatedPages) -> Result<Folio, FtlError> {
        Ok(Self {
            paddr: vaddr2paddr(VAddr::new(pages.as_ptr() as usize).unwrap()).unwrap(),
            len: pages.len(),
            pages: Some(pages),
        })
    }

    pub fn alloc_mmio(paddr: PAddr, len: usize) -> Result<Folio, FtlError> {
        Ok(Self {
            paddr,
            len,
            pages: None,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn paddr(&self) -> PAddr {
        self.paddr
    }
}
