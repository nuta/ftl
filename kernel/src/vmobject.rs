use core::cmp::min;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_types::vmo::SupplyMode;
use ftl_utils::alignment::is_aligned;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::reserve_slot::ReserveSlot;
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

const MAX_COPY_LEN: usize = 64 * 1024;

/// A physical memory page.
struct Page {
    paddr: PAddr,
}

impl Page {
    /// Allocates a zero-filled page.
    fn allocate_zeroed() -> Result<SharedRef<Self>, ErrorCode> {
        Self::allocate_with(PageType::Zeroed)
    }

    /// Allocates a page without zeroing it.
    ///
    /// The caller must initialize the page before mapping it.
    fn allocate_dirty() -> Result<SharedRef<Self>, ErrorCode> {
        Self::allocate_with(PageType::Dirty)
    }

    fn allocate_with(page_type: PageType) -> Result<SharedRef<Self>, ErrorCode> {
        let paddr = PAGE_ALLOCATOR
            .alloc(MIN_PAGE_SIZE, page_type)
            .ok_or(ErrorCode::OutOfMemory)?;

        SharedRef::new(Self { paddr })
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        // SAFETY: This struct owns the page.
        unsafe { PAGE_ALLOCATOR.free(self.paddr, MIN_PAGE_SIZE) };
    }
}

struct Mutable {
    pages: FxHashMap<usize, SharedRef<Page>>,
}

impl Mutable {
    /// Returns the page at the given index. Returns `None` if the page is not present.
    fn get(&mut self, index: usize) -> Option<SharedRef<Page>> {
        self.pages.get(&index).cloned()
    }

    /// Returns the page at the given index. If the page is not present, it
    /// is allocated on demand.
    fn get_or_fill(&mut self, index: usize) -> Result<SharedRef<Page>, ErrorCode> {
        if let Some(page) = self.get(index) {
            return Ok(page);
        }

        // Allocate the slot.
        let slot = self
            .pages
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?;

        // Allocate a page and insert it into the slot.
        let page = Page::allocate_zeroed()?;
        slot.insert(index, page.clone());
        Ok(page)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Zeroed,
    User,
}

/// A virtually-contiguous memory region.
pub struct VmObject {
    source: Source,
    mutable: SpinLock<Mutable>,
    len: usize,
}

impl VmObject {
    pub fn new_anonymous(len: usize) -> Result<SharedRef<Self>, ErrorCode> {
        Self::new(len, Source::Zeroed)
    }

    pub fn new_user(len: usize) -> Result<SharedRef<Self>, ErrorCode> {
        Self::new(len, Source::User)
    }

    fn new(len: usize, source: Source) -> Result<SharedRef<Self>, ErrorCode> {
        if len == 0 || !is_aligned(len, MIN_PAGE_SIZE) {
            return Err(ErrorCode::NotAligned);
        }

        SharedRef::new(Self {
            len,
            source,
            mutable: SpinLock::new(Mutable {
                pages: FxHashMap::new(),
            }),
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    fn get_page(&self, index: usize) -> Result<SharedRef<Page>, ErrorCode> {
        let mut mutable = self.mutable.lock();
        match self.source {
            Source::Zeroed => mutable.get_or_fill(index),
            // User VMOs are filled by the user explicitly. If the page
            // is not present, just return an error.
            Source::User => mutable.get(index).ok_or(ErrorCode::PageAbsent),
        }
    }

    pub fn ensure_page(&self, index: usize) -> Result<PAddr, ErrorCode> {
        if index >= self.len / MIN_PAGE_SIZE {
            return Err(ErrorCode::OutOfBounds);
        }

        let page = self.get_page(index)?;
        Ok(page.paddr)
    }

    /// Fill pages in a user-paged VMO.
    pub fn supply(&self, mode: SupplyMode, offset: usize, uslice: USlice) -> Result<(), ErrorCode> {
        if self.source != Source::User {
            return Err(ErrorCode::Unsupported);
        }

        if !is_aligned(offset, MIN_PAGE_SIZE) || !is_aligned(uslice.len(), MIN_PAGE_SIZE) {
            return Err(ErrorCode::NotAligned);
        }

        let end = offset
            .checked_add(uslice.len())
            .ok_or(ErrorCode::OutOfBounds)?;

        if end > self.len {
            return Err(ErrorCode::OutOfBounds);
        }

        // Fill each page, starting at the offset, from the user slice.
        let mut off = 0;
        while off < uslice.len() {
            let index = (offset + off) / MIN_PAGE_SIZE;

            // Skip present pages.
            if self.mutable.lock().pages.contains_key(&index) {
                off += MIN_PAGE_SIZE;
                continue;
            }

            let page = match mode {
                SupplyMode::Copy => {
                    // Copy the buffer to a new page.
                    // TODO: Copy-on-write to share the same physical page.
                    let page = Page::allocate_dirty()?;
                    let src = uslice.subslice(off, MIN_PAGE_SIZE)?;
                    let page_slice = PageSlice::new(page.clone(), 0, MIN_PAGE_SIZE)?;
                    page_slice.read_user(src)?;
                    page
                }
            };

            // Register the page into the VMO.
            //
            // When the userspace page fault handler resumes the page-faulted
            // thread, it will trigger another page fault on this VMO, kernel
            // finds this new page, and resolves it.
            let mut mutable = self.mutable.lock();
            if !mutable.pages.contains_key(&index) {
                mutable
                    .pages
                    .reserve_slot()
                    .map_err(|_| ErrorCode::OutOfMemory)?
                    .insert(index, page);
            }

            off += MIN_PAGE_SIZE;
        }

        Ok(())
    }

    /// Copies present pages in the range into a new VMO.
    ///
    /// This method copies from `offset` to `offset + len` , and the newly
    /// created VMO will be `len` bytes long.
    pub fn snapshot(&self, offset: usize, len: usize) -> Result<SharedRef<Self>, ErrorCode> {
        if !is_aligned(offset, MIN_PAGE_SIZE) || !is_aligned(len, MIN_PAGE_SIZE) {
            return Err(ErrorCode::NotAligned);
        }

        let end = offset.checked_add(len).ok_or(ErrorCode::OutOfBounds)?;
        if end > self.len {
            return Err(ErrorCode::OutOfBounds);
        }

        // Allocate a new VMO.
        let new_vmo = Self::new(len, self.source)?;

        // Copy each present page in the range.
        // FIXME: If VMO is large, this could block the kernel for a long time.
        let mut new_mutable = new_vmo.mutable.lock();
        let mutable = self.mutable.lock();
        let start_index = offset / MIN_PAGE_SIZE;
        let end_index = end / MIN_PAGE_SIZE;
        for (index, page) in mutable.pages.iter() {
            if !(start_index..end_index).contains(index) {
                // TODO: Use BTreeMap?
                continue;
            }

            // Allocate the slot in the new VMO.
            let slot = new_mutable
                .pages
                .reserve_slot()
                .map_err(|_| ErrorCode::OutOfMemory)?;

            // Copy the page.
            // TODO: Copy-on-write to share the same physical page.
            let new_page = Page::allocate_dirty()?;
            let src: *const u8 = arch::paddr2vaddr(page.paddr).as_ptr();
            let dst: *mut u8 = arch::paddr2vaddr(new_page.paddr).as_mut_ptr();

            // SAFETY: We still have references to the pages, so they won't be
            //         freed while copying.
            unsafe { core::ptr::copy_nonoverlapping(src, dst, MIN_PAGE_SIZE) };

            // Insert the new page into the new VMO.
            slot.insert(index - start_index, new_page);
        }

        drop(new_mutable);
        Ok(new_vmo)
    }

    pub fn read_user(&self, offset: usize, uslice: USlice) -> Result<(), ErrorCode> {
        let mut off = 0;
        self.read_write(offset, uslice.len(), |page_slice| {
            let dst = uslice.subslice(off, page_slice.len())?;
            page_slice.write_user(dst)?;
            off += page_slice.len();
            Ok(())
        })
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
        F: FnMut(PageSlice) -> Result<(), ErrorCode>,
    {
        let end = vmo_offset
            .checked_add(copy_len)
            .ok_or(ErrorCode::OutOfBounds)?;

        if end > self.len {
            return Err(ErrorCode::OutOfBounds);
        }

        let mut remaining = copy_len;
        while remaining > 0 {
            let page_index = vmo_offset / MIN_PAGE_SIZE;
            let page_offset = vmo_offset % MIN_PAGE_SIZE;
            let len = min(remaining, MIN_PAGE_SIZE - page_offset);

            let page = self.get_page(page_index)?;
            let page_slice = PageSlice::new(page, page_offset, len)?;

            // Note: Do not hold the VMO lock before calling the callback.
            //
            // When the callback accesses an unmapped user page, it may cause
            // a page fault on this VMO, causing a dead lock.
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
struct PageSlice {
    /// Keeps the page alive while this slice exists.
    _page: SharedRef<Page>,
    vaddr: VAddr,
    len: usize,
}

impl PageSlice {
    fn new(page: SharedRef<Page>, offset: usize, len: usize) -> Result<Self, ErrorCode> {
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
        // SAFETY: We keep `self._page` alive while copying.
        unsafe { uslice.do_read(self.vaddr.as_mut_ptr(), self.len) }
    }

    fn write_user(&self, uslice: USlice) -> Result<(), ErrorCode> {
        // SAFETY: We keep `self._page` alive while copying.
        unsafe { uslice.do_write(self.vaddr.as_ptr(), self.len) }
    }
}

pub fn sys_vmo_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let len = ctx.a0;

    let vmo = VmObject::new_anonymous(len)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(vmo, rights);
    let id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_vmo_create_user(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let len = ctx.a0;

    let vmo = VmObject::new_user(len)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(vmo, rights);
    let id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_vmo_read(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let offset = ctx.a1;
    let uaddr = UAddr::new(ctx.a2);
    let len = min(ctx.a3, MAX_COPY_LEN);

    let uslice = USlice::new(uaddr, len)?;
    let vmo = current.hspace().get::<VmObject>(id, HandleRight::READ)?;

    vmo.read_user(offset, uslice)?;
    Ok(SyscallOutput::Done(len))
}

pub fn sys_vmo_write(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let offset = ctx.a1;
    let uaddr = UAddr::new(ctx.a2);
    let len = min(ctx.a3, MAX_COPY_LEN);

    let uslice = USlice::new(uaddr, len)?;
    let vmo = current.hspace().get::<VmObject>(id, HandleRight::WRITE)?;

    vmo.write_user(offset, uslice)?;
    Ok(SyscallOutput::Done(len))
}

pub fn sys_vmo_supply(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let offset = ctx.a1;
    let mode = SupplyMode::from_usize(ctx.a2).ok_or(ErrorCode::InvalidArg)?;
    let uaddr = UAddr::new(ctx.a3);
    let len = min(ctx.a4, MAX_COPY_LEN);

    let uslice = USlice::new(uaddr, len)?;
    let vmo = current.hspace().get::<VmObject>(id, HandleRight::WRITE)?;

    vmo.supply(mode, offset, uslice)?;
    Ok(SyscallOutput::Done(len))
}

pub fn sys_vmo_snapshot(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let offset = ctx.a1;
    let len = ctx.a2;

    let hspace = current.hspace();
    let vmo = hspace.get::<VmObject>(id, HandleRight::READ)?;

    let new_vmo = vmo.snapshot(offset, len)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(new_vmo, rights);
    let id = hspace.insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}
