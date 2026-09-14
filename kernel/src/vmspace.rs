use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;
use crate::vmobject::VmObject;

struct Mapping {
    start: UAddr,
    end: UAddr,
    vmo: SharedRef<VmObject>,
    attrs: PageAttrs,
}

impl Mapping {
    pub fn overlaps_with(&self, start: UAddr, end: UAddr) -> bool {
        start < self.end && self.start < end
    }

    pub fn contains(&self, uaddr: UAddr) -> bool {
        self.start <= uaddr && uaddr < self.end
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

    pub fn clone(&self) -> Result<Self, ErrorCode> {
        let new_vmspace = Self::new()?;
        let mutable = self.mutable.lock();
        for mapping in &mutable.mappings {
            new_vmspace.map(mapping.vmo.clone(), mapping.start, mapping.attrs)?;
        }
        Ok(new_vmspace)
    }

    pub fn map(
        &self,
        vmo: SharedRef<VmObject>,
        uaddr: UAddr,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        let allowed_attrs = PageAttrs::READ | PageAttrs::WRITE | PageAttrs::EXEC;
        if !allowed_attrs.contains(attrs) {
            return Err(ErrorCode::InvalidArg);
        }

        if !uaddr.is_aligned_to(MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArg);
        }

        let end = uaddr.add(vmo.len()).ok_or(ErrorCode::OutOfBounds)?;
        if end.as_usize() > arch::USER_ADDR_END {
            return Err(ErrorCode::NotAllowed);
        }

        let mut mutable = self.mutable.lock();
        if mutable
            .mappings
            .iter()
            .any(|mapping| mapping.overlaps_with(uaddr, end))
        {
            return Err(ErrorCode::AlreadyMapped);
        }

        mutable
            .mappings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        // Insert the mapping at the correct position to keep mappings sorted.
        let insert_at = mutable
            .mappings
            .partition_point(|mapping| mapping.start < uaddr);

        mutable.mappings.insert(
            insert_at,
            Mapping {
                start: uaddr,
                end,
                vmo,
                attrs,
            },
        );
        Ok(())
    }

    /// Handles a user page fault in this address space.
    pub fn handle_page_fault(&self, fault_addr: UAddr) -> Result<(), ErrorCode> {
        let aligned_uaddr = UAddr::new(align_down(fault_addr.as_usize(), MIN_PAGE_SIZE));
        let mutable = self.mutable.lock();
        for mapping in &mutable.mappings {
            if mapping.contains(aligned_uaddr) {
                // Found a mapping that contains the fault address.
                let index = (aligned_uaddr.as_usize() - mapping.start.as_usize()) / MIN_PAGE_SIZE;
                let paddr = mapping.vmo.ensure_page(index)?;
                let len = MIN_PAGE_SIZE;
                self.arch.map(aligned_uaddr, paddr, len, mapping.attrs)?;
                return Ok(());
            }
        }

        Err(ErrorCode::OutOfBounds)
    }
}

impl Handleable for VmSpace {}

pub fn sys_vmspace_clone(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let source_id = HandleId::new(ctx.a0);
    let source = current
        .hspace()
        .get::<VmSpace>(source_id, HandleRight::READ)?;
    let vmspace = VmSpace::clone(&source)?;
    let vmspace = SharedRef::new(vmspace)?;
    let rights = HandleRight::READ | HandleRight::WRITE | HandleRight::MAP;
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
    let attrs = PageAttrs::from_raw(ctx.a3);
    let allowed_attrs = PageAttrs::READ | PageAttrs::WRITE | PageAttrs::EXEC;
    if !allowed_attrs.contains(attrs) {
        return Err(ErrorCode::InvalidPageAttrs);
    }

    let hspace = current.hspace();
    let (vmspace, vmo) =
        hspace.get2::<VmSpace, VmObject>(vmspace_id, HandleRight::MAP, vmo_id, HandleRight::MAP)?;

    vmspace.map(vmo, uaddr, attrs)?;
    Ok(SyscallOutput::Done(0))
}
