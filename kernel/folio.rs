use ftl_types::{
    address::{PAddr, VAddr},
    error::FtlError,
};

use crate::arch;

/// Memory pages.
pub struct Folio {
    vaddr: VAddr,
    paddr: Option<PAddr>,
    len: usize,
}

impl Folio {
    pub fn map_paddr(paddr: PAddr, len: usize) -> Result<Folio, FtlError> {
        Ok(Folio {
            vaddr: arch::paddr2vaddr(paddr).ok_or(FtlError::InvalidAddress)?,
            paddr: Some(paddr),
            len,
        })
    }
}
