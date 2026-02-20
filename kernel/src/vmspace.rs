use alloc::vec::Vec;
use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::is_aligned;

use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;
use crate::vmarea::VmArea;

struct Mapping {
    uaddr: usize,
    uaddr_end: usize,
    vmarea: SharedRef<VmArea>,
    attrs: PageAttrs,
}

impl Mapping {
    pub fn overlaps_with(&self, uaddr: usize, uaddr_end: usize) -> bool {
        uaddr < self.uaddr_end && self.uaddr < uaddr_end
    }

    pub fn contains(&self, uaddr: usize) -> bool {
        self.uaddr <= uaddr && uaddr < self.uaddr_end
    }
}

struct Mutable {
    mappings: Vec<Mapping>,
}

pub struct VmSpace {
    arch: arch::VmSpace,
    mutable: SpinLock<Mutable>,
}

impl VmSpace {
    pub fn new() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            arch: arch::VmSpace::new()?,
            mutable: SpinLock::new(Mutable {
                mappings: Vec::new(),
            }),
        })
    }

    pub fn map(
        &self,
        vmarea: SharedRef<VmArea>,
        uaddr: usize,
        attrs: PageAttrs,
    ) -> Result<(), ErrorCode> {
        if !is_aligned(uaddr, arch::MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        let uaddr_end = uaddr
            .checked_add(vmarea.len())
            .ok_or(ErrorCode::OutOfBounds)?;

        let mut mutable = self.mutable.lock();
        if mutable
            .mappings
            .iter()
            .any(|mapping| mapping.overlaps_with(uaddr, uaddr_end))
        {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable.mappings.push(Mapping {
            uaddr,
            uaddr_end,
            vmarea,
            attrs,
        });
        Ok(())
    }

    pub fn handle_page_fault(&self, uaddr: usize, required: PageAttrs) -> Result<(), ErrorCode> {
        let uaddr = align_down(uaddr, arch::MIN_PAGE_SIZE);
        trace!("page fault at {uaddr:#x}");

        let mutable = self.mutable.lock();
        let mapping = mutable
            .mappings
            .iter()
            .find(|mapping| mapping.contains(uaddr))
            .ok_or(ErrorCode::NotFound)?;

        if !mapping.attrs.contains(required) {
            return Err(ErrorCode::NotAllowed);
        }

        let paddr = mapping.vmarea.fill(uaddr - mapping.uaddr)?;
        self.arch
            .map(uaddr, paddr, mapping.vmarea.len(), mapping.attrs)?;
        Ok(())
    }

    pub fn switch(&self) {
        self.arch.switch();
    }
}

impl Handleable for VmSpace {}

impl fmt::Debug for VmSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VmSpace").finish()
    }
}

pub fn sys_vmspace_create(current: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    let vmspace = VmSpace::new()?;
    let id = current
        .process()
        .handle_table()
        .lock()
        .insert(Handle::new(vmspace, HandleRight::ALL))?;
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_vmspace_map(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<SyscallResult, ErrorCode> {
    let vmspace_id = HandleId::from_raw(a0);
    let vmarea_id = HandleId::from_raw(a1);
    let uaddr = a2;
    let attrs = PageAttrs::from_raw(a3);

    let handle_table = current.process().handle_table().lock();
    let vmspace = handle_table
        .get::<VmSpace>(vmspace_id)?
        .authorize(HandleRight::WRITE)?;
    let vmarea = handle_table
        .get::<VmArea>(vmarea_id)?
        .authorize(HandleRight::READ)?;

    vmspace.map(vmarea, uaddr, attrs)?;
    Ok(SyscallResult::Return(0))
}
