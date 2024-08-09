use core::ops::Deref;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;

use crate::handle::OwnedHandle;
use crate::syscall;

struct Folio {
    handle: OwnedHandle,
    vaddr: VAddr,
}

impl Folio {
    fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::folio_vaddr(handle)?;
        Ok(Folio {
            handle: OwnedHandle::from_raw(handle),
            vaddr: VAddr::new(vaddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
        })
    }

    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn vaddr(&self) -> VAddr {
        self.vaddr
    }
}

pub struct MmioFolio {
    folio: Folio,
    paddr: PAddr,
}

impl MmioFolio {
    pub fn create(len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::folio_vaddr(handle)?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            folio: Folio {
                handle: OwnedHandle::from_raw(handle),
                vaddr: VAddr::new(vaddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
            },
            paddr: PAddr::new(paddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
        })
    }

    pub fn create_pinned(paddr: PAddr, len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create_mmio(paddr.as_usize(), len)?;
        let vaddr = syscall::folio_vaddr(handle)?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            folio: Folio {
                handle: OwnedHandle::from_raw(handle),
                vaddr: VAddr::new(vaddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
            },
            paddr: PAddr::new(paddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
        })
    }

    pub fn vaddr(&self) -> VAddr {
        self.folio.vaddr()
    }

    pub fn paddr(&self) -> PAddr {
        self.paddr
    }
}
