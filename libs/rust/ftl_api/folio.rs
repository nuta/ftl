use ftl_types::error::FtlError;

use crate::{handle::OwnedHandle, syscall};

pub struct Folio {
    handle: OwnedHandle,
    vaddr: Option<usize>,
    paddr: Option<usize>,
}

impl Folio {
    pub fn from_handle(handle: OwnedHandle) -> Folio {
        Folio { handle, vaddr: None, paddr: None }
    }

    pub fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        Ok(Self::from_handle(OwnedHandle::from_raw(handle)))
    }

    pub fn create_mmio(paddr: usize, len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create_mmio(paddr, len)?;
        Ok(Self::from_handle(OwnedHandle::from_raw(handle)))
    }

    pub fn paddr(&mut self) -> Result<usize, FtlError> {
        match self.paddr {
            Some(paddr) => Ok(paddr),
            None => {
                let paddr = syscall::folio_paddr(self.handle.id())?;
                self.paddr = Some(paddr);
                Ok(paddr)
            }
        }
    }

    pub fn vaddr(&mut self) -> Result<usize, FtlError> {
        match self.vaddr {
            Some(vaddr) => Ok(vaddr),
            None => {
                let vaddr = syscall::folio_vaddr(self.handle.id())?;
                self.vaddr = Some(vaddr);
                Ok(vaddr)
            }
        }
    }
}
