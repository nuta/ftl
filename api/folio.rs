use ftl_types::error::FtlError;

/// Memory pages.
pub struct Folio {
    vaddr: VAddr,
    paddr: Option<PAddr>,
    len: usize,
}

impl Folio {
    pub fn map_paddr(paddr: PAddr, len: usize) -> Result<Folio, FtlError> {
        todo!()
    }
}
