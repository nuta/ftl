use alloc::vec::Vec;
use core::cmp::min;

use ftl_api::error::ErrorCode;
use ftl_api::handle::HandleRight;
use ftl_api::vmspace::PageAttrs;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::shared_ref::Handleable;
use crate::shared_ref::SharedRef;
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
    /// The mapping sorted by the start address.
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
            return Err(ErrorCode::INVALID_ARG);
        }

        let end = uaddr.add(vmarea.len()).ok_or(ErrorCode::OUT_OF_BOUNDS)?;

        let mut mutable = self.mutable.lock();
        if mutable
            .mappings
            .iter()
            .any(|mapping| mapping.overlaps_with(uaddr, end))
        {
            return Err(ErrorCode::ALREADY_EXISTS);
        }

        mutable
            .mappings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;

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

        // Insert the mapping at the correct position to keep mappings sorted.
        let insert_at = mutable
            .mappings
            .partition_point(|mapping| mapping.start < start);

        mutable.mappings.insert(
            insert_at,
            Mapping {
                start,
                end,
                vmarea,
                attrs,
            },
        );
        Ok(())
    }

    pub fn read_bytes(&self, mut uaddr: UAddr, mut buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.is_empty() {
            return Ok(());
        }

        // Check if the read is out of bounds.
        let _uaddr_end = uaddr.add(buf.len()).ok_or(ErrorCode::OUT_OF_BOUNDS)?;

        let mutable = self.mutable.lock();

        // Do a binary search to find the first mapping.
        let index = mutable
            .mappings
            .partition_point(|mapping| mapping.start <= uaddr)
            .checked_sub(1)
            .ok_or(ErrorCode::OUT_OF_BOUNDS)?;

        // Copy bytes from each vmarea.
        let mut iter = mutable.mappings.iter().skip(index);
        while !buf.is_empty()
            && let Some(mapping) = iter.next()
        {
            if !(mapping.start..mapping.end).contains(&uaddr) {
                return Err(ErrorCode::OUT_OF_BOUNDS);
            }

            let copy_len = min(buf.len(), mapping.end.as_usize() - uaddr.as_usize());
            let (chunk, rest) = buf.split_at_mut(copy_len);
            mapping
                .vmarea
                .read(uaddr.as_usize() - mapping.start.as_usize(), chunk)?;

            // SAFETY: the overflow check above guarantees `uaddr + len` fits.
            uaddr = uaddr.add(copy_len).unwrap();
            buf = rest;
        }

        Ok(())
    }
}

impl Handleable for VmSpace {
    const DEFAULT_RIGHT: HandleRight = HandleRight::READ
        .or(HandleRight::WRITE)
        .or(HandleRight::MAP);
}
