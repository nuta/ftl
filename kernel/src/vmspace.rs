use alloc::vec::Vec;
use core::ops::Range;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::PageFaultInfo;
use ftl_types::thread::SyscallRegs;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::is_aligned;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::quota::Quota;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;
use crate::vmobject::VmObject;

#[derive(Clone)]
struct Mapping {
    start: UAddr,
    end: UAddr,
    vmo: Handle<VmObject>,
    /// The offset in the VMO.
    offset: usize,
    attrs: PageAttrs,
}

impl Mapping {
    pub fn contains(&self, uaddr: UAddr) -> bool {
        self.start <= uaddr && uaddr < self.end
    }
}

fn check_vmo_attrs(allowed: HandleRight, requested: PageAttrs) -> Result<(), ErrorCode> {
    if !allowed.contains(HandleRight::READ) {
        return Err(ErrorCode::NotAllowed);
    }

    if requested.contains(PageAttrs::WRITE) && !allowed.contains(HandleRight::WRITE) {
        return Err(ErrorCode::NotAllowed);
    }

    Ok(())
}

struct Mutable {
    /// The mapping sorted by the start address.
    mappings: Vec<Mapping>,
}

impl Mutable {
    fn find_index(&self, uaddr: UAddr) -> Option<usize> {
        let index = self.mappings.partition_point(|m| m.end <= uaddr);
        let mapping = self.mappings.get(index)?;
        if mapping.contains(uaddr) {
            Some(index)
        } else {
            None
        }
    }

    fn find(&self, uaddr: UAddr) -> Option<&Mapping> {
        let index = self.find_index(uaddr)?;
        Some(&self.mappings[index])
    }

    fn overlapping_range(&self, start: UAddr, end: UAddr) -> Range<usize> {
        let first = self.mappings.partition_point(|m| m.end <= start);
        let last = self.mappings.partition_point(|m| m.start < end);
        first..last
    }

    fn overlapping(&self, start: UAddr, end: UAddr) -> &[Mapping] {
        let range = self.overlapping_range(start, end);
        &self.mappings[range]
    }

    fn overlapping_mut(&mut self, start: UAddr, end: UAddr) -> &mut [Mapping] {
        let range = self.overlapping_range(start, end);
        &mut self.mappings[range]
    }

    /// Splits a mapping at `uaddr` into two mappings.
    ///
    /// Returns `Ok` if `uaddr` is not in any mappings.
    fn split_at(&mut self, uaddr: UAddr) -> Result<(), ErrorCode> {
        // Find the mapping containing uaddr.
        let Some(index) = self.find_index(uaddr) else {
            // No mapping contains uaddr.
            return Ok(());
        };

        if self.mappings[index].start == uaddr {
            // Already split at uaddr.
            return Ok(());
        }

        self.mappings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        // Original mapping.
        let left = &mut self.mappings[index];

        // New mapping.
        let mut right = left.clone();
        right.start = uaddr;
        right.offset += uaddr.as_usize() - left.start.as_usize();

        // Shrink the range of the original mapping.
        left.end = uaddr;

        // Insert the new mapping after the original mapping.
        self.mappings.insert(index + 1, right);
        Ok(())
    }
}

/// A virtual memory space.
pub struct VmSpace {
    arch: arch::VmSpace,
    mutable: SpinLock<Mutable>,
}

impl VmSpace {
    pub fn new(quota: SharedRef<Quota>) -> Result<Self, ErrorCode> {
        let arch = arch::VmSpace::new(quota)?;
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

    pub fn clone(&self, quota: SharedRef<Quota>) -> Result<Self, ErrorCode> {
        let new_vmspace = Self::new(quota)?;
        let mutable = self.mutable.lock();
        for mapping in &mutable.mappings {
            new_vmspace.insert(mapping.clone())?;
        }
        Ok(new_vmspace)
    }

    pub fn map(
        &self,
        vmo: Handle<VmObject>,
        uaddr: UAddr,
        offset: usize,
        len: usize,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        if !is_aligned(offset, MIN_PAGE_SIZE) {
            return Err(ErrorCode::NotAligned);
        }

        let end = validate_mapping_range(uaddr, len)?;
        let vmo_end = offset.checked_add(len).ok_or(ErrorCode::OutOfBounds)?;
        if vmo_end > vmo.object().len() {
            return Err(ErrorCode::OutOfBounds);
        }

        check_vmo_attrs(vmo.rights(), attrs)?;

        self.insert(Mapping {
            start: uaddr,
            end,
            vmo,
            offset,
            attrs,
        })
    }

    fn insert(&self, mapping: Mapping) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !mutable.overlapping(mapping.start, mapping.end).is_empty() {
            return Err(ErrorCode::AlreadyMapped);
        }

        // Insert the mapping at the correct position to keep mappings sorted.
        let insert_at = mutable
            .mappings
            .partition_point(|m| m.start < mapping.start);

        mutable
            .mappings
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?
            .insert(insert_at, mapping);
        Ok(())
    }

    pub fn unmap(&self, uaddr: UAddr, len: usize) -> Result<(), ErrorCode> {
        let end = validate_mapping_range(uaddr, len)?;

        let mut mutable = self.mutable.lock();
        mutable.split_at(uaddr)?;
        mutable.split_at(end)?;

        // Unmap from the page table before removing the mapping.
        self.arch.unmap(uaddr, len)?;

        // Remove the mapping. This decrements page reference counters and may
        // free the pages.
        let range = mutable.overlapping_range(uaddr, end);
        mutable.mappings.drain(range);
        Ok(())
    }

    pub fn permit(&self, uaddr: UAddr, len: usize, attrs: PageAttrs) -> Result<(), ErrorCode> {
        let end = validate_mapping_range(uaddr, len)?;

        let mut mutable = self.mutable.lock();

        // Check if VMO handle rights allow the new attributes.
        for mapping in mutable.overlapping(uaddr, end) {
            check_vmo_attrs(mapping.vmo.rights(), attrs)?;
        }

        // Split the mappings.
        mutable.split_at(uaddr)?;
        mutable.split_at(end)?;

        // Unmap the pages. The page fault handler will map them again lazily.
        //
        // TODO: This is to reduce the arch code, but I think we can update now.
        self.arch.unmap(uaddr, len)?;

        // Update the attributes of the mappings, including the ones that are
        // inbetween `uaddr` and `end`.
        for mapping in mutable.overlapping_mut(uaddr, end) {
            mapping.attrs = attrs;
        }

        Ok(())
    }

    /// Handles a user page fault in this address space.
    pub fn handle_page_fault(
        &self,
        fault_addr: UAddr,
        info: PageFaultInfo,
    ) -> Result<(), ErrorCode> {
        let aligned_uaddr = UAddr::new(align_down(fault_addr.as_usize(), MIN_PAGE_SIZE));
        let mutable = self.mutable.lock();
        let Some(mapping) = mutable.find(aligned_uaddr) else {
            return Err(ErrorCode::BadAccess);
        };

        if !mapping.attrs.contains(info.access()) {
            // The page fault was caused by an invalid access.
            return Err(ErrorCode::BadAccess);
        }

        // Found a mapping that contains the fault address.
        let offset = mapping.offset + (aligned_uaddr.as_usize() - mapping.start.as_usize());
        let index = offset / MIN_PAGE_SIZE;
        let paddr = mapping.vmo.object().ensure_page(index)?;
        let len = MIN_PAGE_SIZE;
        self.arch.map(aligned_uaddr, paddr, len, mapping.attrs)?;
        Ok(())
    }
}

impl Handleable for VmSpace {}

/// Returns `uaddr + len`. Returns an error if they are not valid parameters.
fn validate_mapping_range(uaddr: UAddr, len: usize) -> Result<UAddr, ErrorCode> {
    if len == 0 {
        return Err(ErrorCode::InvalidArg);
    }

    if !uaddr.is_aligned_to(MIN_PAGE_SIZE) || !is_aligned(len, MIN_PAGE_SIZE) {
        return Err(ErrorCode::NotAligned);
    }

    let end = uaddr.add(len).ok_or(ErrorCode::OutOfBounds)?;
    if end.as_usize() > arch::USER_ADDR_END {
        return Err(ErrorCode::NotAllowed);
    }

    Ok(end)
}

pub fn sys_vmspace_clone(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let quota_id = HandleId::new(ctx.a0);
    let source_id = HandleId::new(ctx.a1);
    let (quota, source) = current.hspace().get2::<Quota, VmSpace>(
        quota_id,
        HandleRight::CHARGE,
        source_id,
        HandleRight::READ | HandleRight::WRITE,
    )?;
    let vmspace = VmSpace::clone(&source, quota)?;
    let vmspace = SharedRef::new(vmspace)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(vmspace, rights);
    let id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_vmspace_map(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let vmspace_id = HandleId::new(ctx.a0);
    let vmo_id = HandleId::new(ctx.a1);
    let uaddr = UAddr::new(ctx.a2);
    let offset = ctx.a3;
    let len = ctx.a4;
    let attrs = PageAttrs::from_raw(ctx.a5)?;

    let hspace = current.hspace();
    let vmspace = hspace.get::<VmSpace>(vmspace_id, HandleRight::WRITE)?;
    let vmo = hspace.get_handle::<VmObject>(vmo_id)?;
    vmspace.map(vmo, uaddr, offset, len, attrs)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_vmspace_unmap(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let vmspace_id = HandleId::new(ctx.a0);
    let uaddr = UAddr::new(ctx.a1);
    let len = ctx.a2;
    let vmspace = current
        .hspace()
        .get::<VmSpace>(vmspace_id, HandleRight::WRITE)?;

    vmspace.unmap(uaddr, len)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_vmspace_permit(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let vmspace_id = HandleId::new(ctx.a0);
    let uaddr = UAddr::new(ctx.a1);
    let len = ctx.a2;
    let attrs = PageAttrs::from_raw(ctx.a3)?;
    let vmspace = current
        .hspace()
        .get::<VmSpace>(vmspace_id, HandleRight::WRITE)?;

    vmspace.permit(uaddr, len, attrs)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_vmspace_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let quota_id = HandleId::new(ctx.a0);
    let quota = current
        .hspace()
        .get::<Quota>(quota_id, HandleRight::CHARGE)?;
    let vmspace = VmSpace::new(quota)?;
    let vmspace = SharedRef::new(vmspace)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(vmspace, rights);
    let id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}
