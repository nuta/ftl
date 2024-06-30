use core::ops::Deref;

use ftl_types::error::FtlError;

use crate::handle::OwnedHandle;
use crate::syscall;

pub struct Folio {
    handle: OwnedHandle,
    vaddr: usize,
}

impl Folio {
    pub fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::folio_vaddr(handle)?;
        Ok(Folio {
            handle: OwnedHandle::from_raw(handle),
            vaddr,
        })
    }

    pub fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    pub fn vaddr(&self) -> usize {
        self.vaddr
    }
}

pub struct MmioFolio {
    folio: Folio,
    paddr: usize,
}

impl Deref for MmioFolio {
    type Target = Folio;

    fn deref(&self) -> &Self::Target {
        &self.folio
    }
}

impl MmioFolio {
    pub fn create(paddr: usize, len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create_mmio(paddr, len)?;
        let vaddr = syscall::folio_vaddr(handle)?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            folio: Folio {
                handle: OwnedHandle::from_raw(handle),
                vaddr,
            },
            paddr,
        })
    }

    pub fn paddr(&self) -> usize {
        self.paddr
    }
}
