use alloc::vec::Vec;
use core::fmt;
use core::slice;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_utils::alignment::is_aligned;

use crate::address::PAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::memory::PAGE_ALLOCATOR;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

struct Page {
    paddr: PAddr,
}

impl Page {
    pub fn new(paddr: PAddr) -> Self {
        Self { paddr }
    }

    pub fn as_slice(&self) -> &[u8] {
        let vaddr = arch::paddr2vaddr(self.paddr);
        unsafe { slice::from_raw_parts(vaddr.as_usize() as *const u8, MIN_PAGE_SIZE) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        let vaddr = arch::paddr2vaddr(self.paddr);
        unsafe { slice::from_raw_parts_mut(vaddr.as_usize() as *mut u8, MIN_PAGE_SIZE) }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        // TODO: Free the page.
    }
}

struct Mutable {
    pages: Vec<Option<Page>>,
}

impl Mutable {
    pub fn fill(&mut self, pager: &Pager, index: usize) -> Result<&mut Page, ErrorCode> {
        let slot = &mut self.pages[index];
        match slot {
            Some(page) => Ok(page),
            None => {
                match pager {
                    Pager::Any => {
                        let paddr = PAGE_ALLOCATOR
                            .alloc(MIN_PAGE_SIZE)
                            .ok_or(ErrorCode::OutOfMemory)?;
                        let page = Page::new(paddr);
                        *slot = Some(page);
                        Ok(slot.as_mut().unwrap())
                    }
                }
            }
        }
    }
}

enum Pager {
    Any,
}

pub struct VmArea {
    pager: Pager,
    len: usize,
    mutable: SpinLock<Mutable>,
}

impl VmArea {
    pub fn create_any(len: usize) -> Result<SharedRef<Self>, ErrorCode> {
        if !is_aligned(len, MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        let n = len / MIN_PAGE_SIZE;
        let mut pages = Vec::with_capacity(n);
        for _ in 0..n {
            pages.push(None);
        }

        SharedRef::new(Self {
            pager: Pager::Any,
            len,
            mutable: SpinLock::new(Mutable { pages }),
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn fill(&self, offset: usize) -> Result<PAddr, ErrorCode> {
        if !is_aligned(offset, MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        if offset >= self.len {
            return Err(ErrorCode::OutOfBounds);
        }

        let index = offset / MIN_PAGE_SIZE;
        let mut mutable = self.mutable.lock();
        let page = mutable.fill(&self.pager, index)?;
        Ok(page.paddr)
    }

    pub fn read(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        offset: usize,
        buf: &UserSlice,
    ) -> Result<(), ErrorCode> {
        if !is_aligned(offset, MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        if !is_aligned(buf.len(), MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        let base = offset / MIN_PAGE_SIZE;
        let n = buf.len() / MIN_PAGE_SIZE;
        let mut mutable = self.mutable.lock();
        for i in 0..n {
            let slice = buf.subslice(i * MIN_PAGE_SIZE, MIN_PAGE_SIZE)?;
            let page = mutable.fill(&self.pager, base + i)?;
            isolation.write_bytes(&slice, page.as_slice())?;
        }

        Ok(())
    }

    pub fn write(
        &self,
        isolation: &SharedRef<dyn Isolation>,
        offset: usize,
        buf: &UserSlice,
    ) -> Result<(), ErrorCode> {
        if !is_aligned(offset, MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        if !is_aligned(buf.len(), MIN_PAGE_SIZE) {
            return Err(ErrorCode::InvalidArgument);
        }

        let base = offset / MIN_PAGE_SIZE;
        let n = buf.len() / MIN_PAGE_SIZE;
        let mut mutable = self.mutable.lock();
        for i in 0..n {
            let slice = buf.subslice(i * MIN_PAGE_SIZE, MIN_PAGE_SIZE)?;
            let page = mutable.fill(&self.pager, base + i)?;
            isolation.read_bytes(&slice, page.as_mut_slice())?;
        }

        Ok(())
    }
}

impl Handleable for VmArea {}

impl fmt::Debug for VmArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VmArea").finish()
    }
}

pub fn sys_vmarea_create(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let len = a0;

    let vmarea = VmArea::create_any(len)?;
    let mut handle_table = current.process().handle_table().lock();
    let id = handle_table.insert(Handle::new(vmarea, HandleRight::ALL))?;
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_vmarea_read(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<SyscallResult, ErrorCode> {
    let id = HandleId::from_raw(a0);
    let offset = a1;
    let buf = UserSlice::new(UserPtr::new(a2), a3)?;

    let process = current.process();
    process
        .handle_table()
        .lock()
        .get::<VmArea>(id)?
        .authorize(HandleRight::READ)?
        .read(process.isolation(), offset, &buf)?;

    Ok(SyscallResult::Return(0))
}

pub fn sys_vmarea_write(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<SyscallResult, ErrorCode> {
    let id = HandleId::from_raw(a0);
    let offset = a1;
    let buf = UserSlice::new(UserPtr::new(a2), a3)?;

    let process = current.process();
    process
        .handle_table()
        .lock()
        .get::<VmArea>(id)?
        .authorize(HandleRight::WRITE)?
        .write(process.isolation(), offset, &buf)?;

    Ok(SyscallResult::Return(0))
}
