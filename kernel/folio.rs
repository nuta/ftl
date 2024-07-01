use core::borrow::Borrow;

use ftl_types::address::PAddr;
use ftl_types::error::FtlError;

use crate::arch;
use crate::cpuvar::current_thread;
use crate::memory::AllocPagesError;
use crate::memory::AllocatedPages;

enum Pages {
    Anonymous(AllocatedPages),
    Mmio { paddr: PAddr },
}

pub struct Folio {
    #[allow(dead_code)]
    pages: Pages,
    len: usize,
}

impl Folio {
    pub fn alloc(len: usize) -> Result<Folio, AllocPagesError> {
        let pages = AllocatedPages::alloc(len)?;
        Ok(Self::from_allocated_pages(pages))
    }

    pub fn alloc_mmio(paddr: PAddr, len: usize) -> Result<Folio, AllocPagesError> {
        Ok(Folio {
            pages: Pages::Mmio { paddr },
            len,
        })
    }

    pub fn from_allocated_pages(pages: AllocatedPages) -> Folio {
        let len = pages.len();
        Folio {
            pages: Pages::Anonymous(pages),
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn vaddr(&self) -> Result<usize, FtlError> {
        if current_thread().borrow().process().is_kernel_process() {
            match &self.pages {
                Pages::Anonymous(pages) => Ok(pages.as_ptr() as usize),
                Pages::Mmio { paddr } => {
                    let vaddr = arch::paddr2vaddr(*paddr).ok_or(FtlError::InvalidArg)?;
                    Ok(vaddr.as_usize())
                }
            }
        } else {
            Err(FtlError::NotSupported)
        }
    }

    pub fn paddr(&self) -> Result<usize, FtlError> {
        match &self.pages {
            Pages::Mmio { paddr, .. } => Ok(paddr.as_usize()),
            _ => Err(FtlError::NotSupported),
        }
    }
}
