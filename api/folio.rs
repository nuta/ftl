use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;

/// Memory pages.
pub struct Folio {
    // FIXME: Support user-space mode.
    raw: ftl_kernel::folio::Folio,
}

impl Folio {
    pub fn map_paddr(paddr: PAddr, len: usize) -> Result<Folio, FtlError> {
        Ok(Folio {
            raw: ftl_kernel::folio::Folio::map_paddr(paddr, len)?,
        })
    }

    pub fn alloc(len: usize) -> Result<Folio, FtlError> {
        Ok(Folio {
            raw: ftl_kernel::folio::Folio::alloc(len)?,
        })
    }

    pub fn vaddr(&self) -> VAddr {
        self.raw.vaddr()
    }

    pub fn paddr(&self) -> PAddr {
        self.raw.paddr()
    }
}
