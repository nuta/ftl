use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_types::vmspace::PageAttrs;
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
        if !uaddr.is_aligned_to(MIN_PAGE_SIZE) {
            return Err(ErrorCode::INVALID_ARG);
        }

        let end = uaddr.add(vmo.len()).ok_or(ErrorCode::OUT_OF_BOUNDS)?;

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
        let num_pages = vmo.len() / MIN_PAGE_SIZE;
        let start = uaddr;
        let mut uaddr = uaddr;
        for index in 0..num_pages {
            let paddr = vmo.ensure_page(index)?;
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
                vmo,
                attrs,
            },
        );
        Ok(())
    }
}

impl Handleable for VmSpace {}

pub fn sys_vmspace_clone(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let source_id = HandleId::new(ctx.a0);
    let source = current
        .isolate()
        .handles()
        .lock()
        .get::<VmSpace>(source_id, HandleRight::READ)?;
    let vmspace = VmSpace::clone(&source)?;
    let vmspace = SharedRef::new(vmspace)?;
    let rights = HandleRight::READ | HandleRight::WRITE | HandleRight::MAP;
    let handle = Handle::new(vmspace, rights);
    let id = current.isolate().handles().lock().insert(handle)?;
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

    let handles = current.isolate().handles().lock();
    let vmspace = handles.get::<VmSpace>(vmspace_id, HandleRight::MAP)?;
    let vmo = handles.get::<VmObject>(vmo_id, HandleRight::MAP)?;
    drop(handles);

    vmspace.map(vmo, uaddr, attrs)?;
    Ok(SyscallOutput::Done(0))
}
