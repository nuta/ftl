use alloc::vec::Vec;
use core::cmp::min;
use core::ptr;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::vcpu::SyscallRegs;
use ftl_utils::alignment::is_aligned;
use ftl_utils::spinlock::SpinLock;

use crate::address::PAddr;
use crate::arch;
use crate::arch::MIN_PAGE_SIZE;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::isolate::Isolate;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::vcpu::VCpu;

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

    pub fn write(&self, mut offset: usize, mut data: &[u8]) -> Result<(), ErrorCode> {
        let offset_end = offset
            .checked_add(data.len())
            .ok_or(ErrorCode::OUT_OF_BOUNDS)?;
        if offset_end > self.len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let mut mutable = self.mutable.lock();
        while !data.is_empty() {
            let index = offset / MIN_PAGE_SIZE;
            let page_offset = offset % MIN_PAGE_SIZE;
            let copy_len = min(data.len(), MIN_PAGE_SIZE - page_offset);

            let page = mutable.get_or_fill(index)?;
            let vaddr = arch::paddr2vaddr(page.paddr);

            unsafe {
                let dst = vaddr.as_mut_ptr::<u8>().add(page_offset);
                ptr::copy_nonoverlapping(data.as_ptr(), dst, copy_len);
            }

            data = &data[copy_len..];
            offset += copy_len;
        }

        Ok(())
    }

    /// Reads bytes into the buffer.
    ///
    /// The lazily-allocated pages are filled on demand.
    pub fn read(&self, mut offset: usize, mut buf: &mut [u8]) -> Result<(), ErrorCode> {
        let offset_end = offset
            .checked_add(buf.len())
            .ok_or(ErrorCode::OUT_OF_BOUNDS)?;

        if offset_end > self.len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let mut mutable = self.mutable.lock();
        while !buf.is_empty() {
            let index = offset / MIN_PAGE_SIZE;
            let offset_in_page = offset % MIN_PAGE_SIZE;

            let page = mutable.get_or_fill(index)?;
            let vaddr = arch::paddr2vaddr(page.paddr);

            let copy_len = min(buf.len(), MIN_PAGE_SIZE - offset_in_page);
            unsafe {
                let src = vaddr.as_ptr::<u8>().add(offset_in_page);
                ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), copy_len);
            }

            buf = &mut buf[copy_len..];
            offset += copy_len;
        }

        Ok(())
    }
}

impl Handleable for VmObject {}

pub fn sys_vmo_create(
    current: &SharedRef<VCpu>,
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
