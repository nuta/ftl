use alloc::vec::Vec;

use crate::address::UAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::arch::PageAttrs;
use crate::error::ErrorCode;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::vmarea::VmArea;

struct Mapping {
    start: UAddr,
    end: UAddr,
    vmarea: SharedRef<VmArea>,
    attrs: PageAttrs,
}

impl Mapping {
    pub fn overlaps_with(&self, start: UAddr, end: UAddr) -> bool {
        start < self.end && self.start < end
    }
}

struct Mutable {
    mappings: Vec<Mapping>,
}

/// A virtual memory space.
pub struct VmSpace {
    arch: arch::VmSpace,
    mutable: SpinLock<Mutable>,
}

impl VmSpace {
    pub fn new() -> Result<Self, ErrorCode> {
        let arch = arch::VmSpace::new()?;
        Ok(Self {
            arch,
            mutable: SpinLock::new(Mutable {
                mappings: Vec::new(),
            }),
        })
    }

    pub fn switch(&self) {
        self.arch.switch();
    }

    pub fn map(
        &self,
        vmarea: SharedRef<VmArea>,
        uaddr: UAddr,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        if !uaddr.is_aligned_to(MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        let end = uaddr.add(vmarea.len()).ok_or(ErrorCode::OutOfBounds)?;

        let mut mutable = self.mutable.lock();
        if mutable
            .mappings
            .iter()
            .any(|mapping| mapping.overlaps_with(uaddr, end))
        {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable
            .mappings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        // Map the VM area to the virtual address space.
        // TODO: Map lazily when pages are accessed.
        let num_pages = vmarea.len() / MIN_PAGE_SIZE;
        let start = uaddr;
        let mut uaddr = uaddr;
        for index in 0..num_pages {
            let paddr = vmarea.ensure_page(index)?;
            self.arch.map(uaddr, paddr, MIN_PAGE_SIZE, attrs)?;
            // SAFETY: `end` guarantees that `uaddr` will not overflow.
            uaddr = uaddr.add(MIN_PAGE_SIZE).unwrap();
        }

        mutable.mappings.push(Mapping {
            start,
            end,
            vmarea,
            attrs,
        });
        Ok(())
    }
}
