use ftl_types::address::PAddr;

/// Folio, a set memory "pages", or a contigous page-size-aligned memory
/// region.
pub struct Folio {
    /// The base address of the folio.
    paddr: PAddr,
    /// The size of the folio in bytes.
    size: usize,
}
