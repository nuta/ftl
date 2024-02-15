use ftl_types::{
    address::{PAddr, VAddr},
    error::FtlError,
};

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
}
