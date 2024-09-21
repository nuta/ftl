//! A contiguous page-aliged memory block.
use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::vmspace::PageProtect;

use crate::handle::OwnedHandle;
use crate::start::app_vmspace_handle;
use crate::syscall;

/// The ownership of a contiguous page-aliged memory region.
///
/// # Prefer [`Box<T>`] over folio
///
/// Unless you need low-level control over memory allocation, use containers
/// like [`Vec<T>`] or [`Box<T>`] instead of folio. Folio is intended for
/// OS services that need to manage memory regions directly, such as DMA
/// buffers, MMIO regions, and shared memory between processes.
///
/// # Key facts
///
/// - The memory block address is page-aligned (typically 4KB).
/// - The memory block size is also page-aligned.
/// - The memory block is physically contiguous.
///
/// # Why "folio"?
///
/// Because it's *a sheet of paper (pages)*.
pub struct Folio {
    handle: OwnedHandle,
}

impl Folio {
    pub fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        Ok(Folio {
            handle: OwnedHandle::from_raw(handle),
        })
    }

    pub fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    pub fn paddr(&self) -> Result<PAddr, FtlError> {
        let paddr = syscall::folio_paddr(self.handle.id())?;
        let paddr = PAddr::new(paddr);
        Ok(paddr)
    }
}

/// A folio mapped to the current process's address space.
pub struct MmioFolio {
    _folio: Folio,
    paddr: PAddr,
    vaddr: VAddr,
}

impl MmioFolio {
    /// Allocates a folio at an arbitrary physical address, and maps it to the
    /// current process's address space.
    pub fn create(len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let vaddr = syscall::vmspace_map(
            app_vmspace_handle(),
            len,
            handle,
            PageProtect::READABLE | PageProtect::WRITABLE,
        )?;
        let paddr = syscall::folio_paddr(handle)?;
        Ok(MmioFolio {
            _folio: Folio {
                handle: OwnedHandle::from_raw(handle),
            },
            paddr: PAddr::new(paddr),
            vaddr,
        })
    }

    /// Allocates a folio at a specific physical address (`paddr`), and maps it to the
    /// current process's address space.
    pub fn create_pinned(paddr: PAddr, len: usize) -> Result<MmioFolio, FtlError> {
        let handle = syscall::folio_create_fixed(paddr, len)?;
        let vaddr = syscall::vmspace_map(
            app_vmspace_handle(),
            len,
            handle,
            PageProtect::READABLE | PageProtect::WRITABLE,
        )?;

        Ok(MmioFolio {
            _folio: Folio {
                handle: OwnedHandle::from_raw(handle),
            },
            paddr,
            vaddr,
        })
    }

    /// Returns the start address of the folio in the current process's address space.
    pub fn vaddr(&self) -> VAddr {
        self.vaddr
    }

    /// Returns the start address of the folio in physical memory space.
    pub fn paddr(&self) -> PAddr {
        self.paddr
    }
}
