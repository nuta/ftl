use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::vmspace::PageProtect;

use crate::handle::OwnedHandle;
use crate::syscall;

struct Folio {
    handle: OwnedHandle,
}

impl Folio {
    fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        Ok(Folio {
            handle: OwnedHandle::from_raw(handle),
        })
    }

    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn paddr(&self) -> Result<PAddr, FtlError> {
        let paddr = syscall::folio_paddr(self.handle.id())?;
        let paddr = PAddr::new(paddr).ok_or(FtlError::InvalidSyscallReturnValue)?;
        Ok(paddr)
    }
}

pub struct MmioFolio {
    folio: Folio,
    paddr: PAddr,
    vaddr: VAddr,
}

impl MmioFolio {
    pub fn create(len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::vmspace_map(handle, len, handle, PageProtect::READABLE | PageProtect::WRITABLE)?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            folio: Folio {
                handle: OwnedHandle::from_raw(handle),
            },
            paddr: PAddr::new(paddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
            vaddr,
        })
    }

    pub fn create_pinned(paddr: PAddr, len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::vmspace_map(handle, len, handle, PageProtect::READABLE | PageProtect::WRITABLE)?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            folio: Folio {
                handle: OwnedHandle::from_raw(handle),
            },
            paddr: PAddr::new(paddr).ok_or(FtlError::InvalidSyscallReturnValue)?,
            vaddr,
        })
    }

    pub fn vaddr(&self) -> VAddr {
        self.vaddr
    }

    pub fn paddr(&self) -> PAddr {
        self.paddr
    }
}
