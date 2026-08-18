use alloc::vec::Vec;
use core::cmp::min;
use core::ptr;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_utils::alignment::is_aligned;
use ftl_utils::spinlock::SpinLock;

use crate::address::PAddr;
use crate::address::UAddr;
use crate::address::USlice;
use crate::address::VAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::isolate::Isolate;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

/// A physical memory page.
struct Page {
    paddr: PAddr,
}

/// A page initializer.
enum Pager {
    /// Pages are filled with zeros.
    Anonymous,
}

struct Mutable {
    pages: Vec<Option<Page>>,
}

impl Mutable {
    fn get_or_fill(&mut self, index: usize) -> Result<&mut Page, ErrorCode> {
        let page = &mut self.pages[index];
        if page.is_none() {
            let paddr = PAGE_ALLOCATOR
                .alloc(MIN_PAGE_SIZE, PageType::Zeroed)
                .ok_or(ErrorCode::OUT_OF_MEMORY)?;
            *page = Some(Page { paddr });
        }

        // SAFETY: We always fill the page if it is none.
        Ok(unsafe { page.as_mut().unwrap_unchecked() })
    }
}

/// A virtually-contiguous memory region.
pub struct VmObject {
    mutable: SpinLock<Mutable>,
    pager: Pager,
    len: usize,
}

impl VmObject {
    pub fn new_anonymous(len: usize) -> Result<SharedRef<Self>, ErrorCode> {
        if len == 0 || !is_aligned(len, MIN_PAGE_SIZE) {
            return Err(ErrorCode::INVALID_ARG);
        }

        //　Mark all pages as empty.
        let mut pages = Vec::new();
        let n = len / MIN_PAGE_SIZE;
        if pages.try_reserve_exact(n).is_err() {
            return Err(ErrorCode::OUT_OF_MEMORY);
        }
        pages.resize_with(n, Default::default);

        SharedRef::new(Self {
            pager: Pager::Anonymous,
            len,
            mutable: SpinLock::new(Mutable { pages }),
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn ensure_page(&self, index: usize) -> Result<PAddr, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if index >= mutable.pages.len() {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let page = mutable.get_or_fill(index)?;
        Ok(page.paddr)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        let mut buf_offset = 0;
        self.read_write(offset, buf.len(), |vaddr, len| {
            unsafe {
                let dst = vaddr.as_mut_ptr::<u8>();
                let src = buf.as_ptr().add(buf_offset);
                ptr::copy_nonoverlapping(src, dst, len);
            }
            buf_offset += len;
            Ok(())
        })
    }

    pub fn write_user(&self, offset: usize, uslice: USlice) -> Result<(), ErrorCode> {
        let mut buf_offset = 0;
        self.read_write(offset, uslice.len(), |vaddr, len| {
            let dst = unsafe { core::slice::from_raw_parts_mut(vaddr.as_mut_ptr(), len) };
            uslice.read_at(buf_offset, dst)?;
            buf_offset += len;
            Ok(())
        })
    }

    /// Reads bytes into the buffer.
    ///
    /// The lazily-allocated pages are filled on demand.
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), ErrorCode> {
        let mut buf_offset = 0;
        self.read_write(offset, buf.len(), |vaddr, len| {
            unsafe {
                let src = vaddr.as_ptr::<u8>();
                let dst = buf.as_mut_ptr().add(buf_offset);
                ptr::copy_nonoverlapping(src, dst, len);
            }
            buf_offset += len;
            Ok(())
        })
    }

    /// Visits the memory region in page-aligned chunks.
    ///
    /// `vmo_offset` and `copy_len` don't need to be page-aligned.
    fn read_write<F>(
        &self,
        mut vmo_offset: usize,
        copy_len: usize,
        mut f: F,
    ) -> Result<(), ErrorCode>
    where
        F: FnMut(VAddr, usize) -> Result<(), ErrorCode>,
    {
        let end = vmo_offset
            .checked_add(copy_len)
            .ok_or(ErrorCode::OUT_OF_BOUNDS)?;

        if end > self.len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let mut mutable = self.mutable.lock();
        let mut remaining = copy_len;
        while remaining > 0 {
            let page_index = vmo_offset / MIN_PAGE_SIZE;
            let page_offset = vmo_offset % MIN_PAGE_SIZE;
            let len = min(remaining, MIN_PAGE_SIZE - page_offset);

            let page = mutable.get_or_fill(page_index)?;
            let vaddr = arch::paddr2vaddr(page.paddr)
                .add(page_offset)
                .ok_or(ErrorCode::OUT_OF_BOUNDS)?;

            f(vaddr, len)?;

            vmo_offset += len;
            remaining -= len;
        }

        Ok(())
    }
}

impl Handleable for VmObject {}

pub fn sys_vmo_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let len = ctx.a1;

    let isolate = current
        .isolate()
        .handles()
        .lock()
        .get::<Isolate>(id, HandleRight::WRITE)?;

    let vmo = VmObject::new_anonymous(len)?;
    let rights = HandleRight::READ | HandleRight::WRITE | HandleRight::MAP;
    let handle = Handle::new(vmo, rights);
    let id = isolate.handles().lock().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_vmo_write(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let offset = ctx.a1;
    let uaddr = UAddr::new(ctx.a2);
    let len = ctx.a3;

    let uslice = USlice::new(uaddr, len)?;
    let isolate = current
        .isolate()
        .handles()
        .lock()
        .get::<Isolate>(id, HandleRight::WRITE)?;

    let vmo = isolate
        .handles()
        .lock()
        .get::<VmObject>(id, HandleRight::WRITE)?;
    vmo.write_user(offset, uslice)?;
    Ok(SyscallOutput::Done(0))
}
