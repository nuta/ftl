use alloc::vec::Vec;
use core::cmp::min;

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
                .ok_or(ErrorCode::OutOfMemory)?;
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
            return Err(ErrorCode::InvalidArg);
        }

        //　Mark all pages as empty.
        let mut pages = Vec::new();
        let n = len / MIN_PAGE_SIZE;
        if pages.try_reserve_exact(n).is_err() {
            return Err(ErrorCode::OutOfMemory);
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
            return Err(ErrorCode::OutOfBounds);
        }

        let page = mutable.get_or_fill(index)?;
        Ok(page.paddr)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<(), ErrorCode> {
        let mut off = 0;
        self.read_write(offset, buf.len(), |page_slice| {
            page_slice.write(&buf[off..off + page_slice.len()])?;
            off += page_slice.len();
            Ok(())
        })
    }

    pub fn write_user(&self, offset: usize, uslice: USlice) -> Result<(), ErrorCode> {
        let mut off = 0;
        self.read_write(offset, uslice.len(), |page_slice| {
            let src = uslice.subslice(off, page_slice.len())?;
            page_slice.read_user(src)?;
            off += page_slice.len();
            Ok(())
        })
    }

    /// Reads bytes into the buffer.
    ///
    /// The lazily-allocated pages are filled on demand.
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), ErrorCode> {
        let mut off = 0;
        self.read_write(offset, buf.len(), |page_slice| {
            page_slice.read(&mut buf[off..off + page_slice.len()])?;
            off += page_slice.len();
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
        F: FnMut(PageSlice<'_>) -> Result<(), ErrorCode>,
    {
        let end = vmo_offset
            .checked_add(copy_len)
            .ok_or(ErrorCode::OutOfBounds)?;

        if end > self.len {
            return Err(ErrorCode::OutOfBounds);
        }

        let mut mutable = self.mutable.lock();
        let mut remaining = copy_len;
        while remaining > 0 {
            let page_index = vmo_offset / MIN_PAGE_SIZE;
            let page_offset = vmo_offset % MIN_PAGE_SIZE;
            let len = min(remaining, MIN_PAGE_SIZE - page_offset);

            let page = mutable.get_or_fill(page_index)?;
            let page_slice = PageSlice::new(page, page_offset, len)?;
            f(page_slice)?;

            vmo_offset += len;
            remaining -= len;
        }

        Ok(())
    }
}

impl Handleable for VmObject {}

/// A slice of a page.
///
/// This provides access without creating a Rust reference to the page data,
/// which might also be mapped into userspace.
struct PageSlice<'a> {
    /// Keeps the page borrowed while this slice exists.
    _page: &'a Page,
    vaddr: VAddr,
    len: usize,
}

impl<'a> PageSlice<'a> {
    fn new(page: &'a Page, offset: usize, len: usize) -> Result<Self, ErrorCode> {
        let end = offset.checked_add(len).ok_or(ErrorCode::OutOfBounds)?;
        if end > MIN_PAGE_SIZE {
            return Err(ErrorCode::OutOfBounds);
        }

        let vaddr = arch::paddr2vaddr(page.paddr)
            .add(offset)
            .ok_or(ErrorCode::OutOfBounds)?;

        Ok(Self {
            _page: page,
            vaddr,
            len,
        })
    }

    fn len(&self) -> usize {
        self.len
    }

    fn read(&self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() != self.len {
            return Err(ErrorCode::InvalidArg);
        }

        unsafe {
            let src = self.vaddr.as_ptr();
            let dst = buf.as_mut_ptr();
            core::ptr::copy(src, dst, self.len);
        }

        Ok(())
    }

    fn write(&self, buf: &[u8]) -> Result<(), ErrorCode> {
        if buf.len() != self.len {
            return Err(ErrorCode::InvalidArg);
        }

        unsafe {
            let src = buf.as_ptr();
            let dst = self.vaddr.as_mut_ptr();
            core::ptr::copy(src, dst, self.len);
        }

        Ok(())
    }

    fn read_user(&self, uslice: USlice) -> Result<(), ErrorCode> {
        // SAFETY: We keep `self._page` borrowed while copying.
        unsafe { uslice.read(self.vaddr.as_mut_ptr(), self.len) }
    }
}

pub fn sys_vmo_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let len = ctx.a0;

    let vmo = VmObject::new_anonymous(len)?;
    let rights = HandleRight::READ | HandleRight::WRITE | HandleRight::MAP;
    let handle = Handle::new(vmo, rights);
    let id = current.isolate().handles().lock().insert(handle)?;
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
    let vmo = current
        .isolate()
        .handles()
        .lock()
        .get::<VmObject>(id, HandleRight::WRITE)?;

    vmo.write_user(offset, uslice)?;
    Ok(SyscallOutput::Done(0))
}
