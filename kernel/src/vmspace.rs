use alloc::vec::Vec;
use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::is_aligned;

use crate::address::PAddr;
use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::UserSlice;
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

    fn fill(&self, uaddr: usize, required: PageAttrs) -> Result<(PAddr, PageAttrs), ErrorCode> {
        if !is_aligned(uaddr, arch::MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

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
        Ok((paddr, mapping.attrs))
    }

    pub fn handle_page_fault(&self, uaddr: usize, required: PageAttrs) -> Result<(), ErrorCode> {
        let uaddr = align_down(uaddr, arch::MIN_PAGE_SIZE);
        let (paddr, attrs) = self.fill(uaddr, required)?;
        self.arch.map(uaddr, paddr, arch::MIN_PAGE_SIZE, attrs)?;
        Ok(())
    }

    pub fn switch(&self) {
        self.arch.switch();
    }
}

pub enum PageChunk<'a> {
    Kernel {
        uaddr: usize,
        len: usize,
    },
    User {
        vmspace: &'a VmSpace,
        uaddr: usize,
        /// The offset within the page.
        offset: usize,
        len: usize,
    },
}

impl<'a> PageChunk<'a> {
    pub fn slice(&self, required: PageAttrs) -> Result<&mut [u8], ErrorCode> {
        let (ptr, len) = match self {
            PageChunk::Kernel { uaddr, len } => (*uaddr, *len),
            PageChunk::User {
                vmspace,
                uaddr,
                offset,
                len,
            } => {
                let (paddr, _) = vmspace.fill(*uaddr, required)?;
                let vaddr = arch::paddr2vaddr(paddr);
                (vaddr.as_usize() + *offset, *len)
            }
        };

        Ok(unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, len) })
    }
}

pub struct PageIter<'a> {
    vmspace: &'a VmSpace,
    uaddr: usize,
    uaddr_end: usize,
    kernel: bool,
}

impl<'a> PageIter<'a> {
    pub fn new(vmspace: &'a VmSpace, slice: &UserSlice) -> Self {
        let start = slice.start.as_usize();
        let end = slice.end.as_usize();
        Self {
            vmspace,
            uaddr: start,
            uaddr_end: end,
            kernel: start >= arch::KERNEL_BASE,
        }
    }
}

impl<'a> Iterator for PageIter<'a> {
    type Item = PageChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.uaddr >= self.uaddr_end {
            return None;
        }

        if self.kernel {
            let chunk = PageChunk::Kernel {
                uaddr: self.uaddr,
                len: self.uaddr_end - self.uaddr,
            };
            self.uaddr = self.uaddr_end;
            return Some(chunk);
        }

        let page_offset = self.uaddr % arch::MIN_PAGE_SIZE;
        let page_uaddr = self.uaddr - page_offset;
        let chunk_len = (arch::MIN_PAGE_SIZE - page_offset).min(self.uaddr_end - self.uaddr);
        let chunk = PageChunk::User {
            vmspace: self.vmspace,
            uaddr: page_uaddr,
            offset: page_offset,
            len: chunk_len,
        };
        self.uaddr += chunk_len;
        Some(chunk)
    }
}

impl Handleable for VmSpace {}

impl fmt::Debug for VmSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VmSpace").finish()
    }
}

pub fn sys_vmspace_create(current: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    let mut handle_table = current.process().handle_table().lock();
    let reserve = handle_table.reserve()?;

    let vmspace = VmSpace::new()?;
    let id = reserve.insert(Handle::new(vmspace, HandleRight::ALL));
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
