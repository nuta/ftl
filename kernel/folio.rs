use crate::memory::AllocPagesError;
use crate::memory::AllocatedPages;

enum Pages {
    Allocated(AllocatedPages),
    Pinned { paddr: usize, len: usize },
}

pub struct Folio {
    #[allow(dead_code)]
    pages: Pages,
}

impl Folio {
    pub fn alloc(len: usize) -> Result<Folio, AllocPagesError> {
        let pages = AllocatedPages::alloc(len)?;
        Ok(Folio {
            pages: Pages::Allocated(pages),
        })
    }

    pub fn alloc_pinned(paddr: usize, len: usize) -> Result<Folio, AllocPagesError> {
        Ok(Folio {
            pages: Pages::Pinned { paddr, len },
        })
    }

    pub fn allocated_pages(&self) -> &AllocatedPages {
        &self.pages
    }

    pub fn allocated_pages_mut(&mut self) -> &mut AllocatedPages {
        &mut self.pages
    }
}
