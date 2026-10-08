use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::max;
use core::cmp::min;
use core::mem::MaybeUninit;
use core::slice;

use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::error::ErrorCode;
use ftl_types::thread::PageFaultInfo;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::alignment::is_aligned;
use ftl_utils::spinlock::SpinLock;

use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::auxv::AT_PAGESZ;
use crate::types::sys::auxv::AT_PHDR;
use crate::types::sys::auxv::AT_PHENT;
use crate::types::sys::auxv::AT_PHNUM;
use crate::types::sys::auxv::AT_RANDOM;
use crate::types::sys::auxv::AT_RANDOM_LEN;
use crate::types::sys::mman::PROT_EXEC;
use crate::types::sys::mman::PROT_READ;
use crate::types::sys::mman::PROT_WRITE;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;

pub(crate) const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_BOTTOM: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;
const BRK_END: usize = STACK_BOTTOM;
const MMAP_START: usize = STACK_BOTTOM + STACK_SIZE;
const USER_END: usize = 0x3000_0000;

#[derive(Clone, Copy)]
struct Brk {
    /// The start of the heap.
    start: usize,
    /// The end of the heap.
    end: usize,
    /// The current break address.
    current: usize,
    /// The current break address, aligned to the page boundary.
    current_aligned: usize,
}

impl Brk {
    fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            current: start,
            current_aligned: start,
        }
    }
}

// TODO: Do we really need to track mappings in LX? Can't we just use
//       kernel's mappings?
#[derive(Clone)]
struct Mapping {
    start: usize,
    len: usize,
    attrs: PageAttrs,
    vmo: Arc<Vmo>,
    /// The offset in the VMO.
    offset: usize,
    /// The backing file.
    file: Option<BackingFile>,
}

#[derive(Clone)]
struct BackingFile {
    file: Arc<dyn FileLike>,
    /// The start offset in the file.
    offset: usize,
    /// The end offset in the file.
    offset_end: usize,
}

impl Mapping {
    fn new(start: usize, len: usize, attrs: PageAttrs, vmo: Vmo) -> Self {
        Self {
            start,
            len,
            attrs,
            vmo: Arc::new(vmo),
            offset: 0,
            file: None,
        }
    }

    fn page_filler(&self, addr: usize) -> Option<PageFiller> {
        let Some(backing_file) = &self.file else {
            // Not a file-backed mapping (zeroed pages). We don't need to fill
            // it, as kernel will do it automatically.
            return None;
        };

        let mapping_offset = addr - self.start;
        let vmo_offset = self.offset + mapping_offset;
        let file_offset = backing_file.offset + mapping_offset;
        let len = min(
            PAGE_SIZE,
            backing_file.offset_end.saturating_sub(file_offset),
        );

        Some(PageFiller {
            vmo: self.vmo.clone(),
            file: backing_file.file.clone(),
            file_offset,
            vmo_offset,
            len,
        })
    }

    fn end(&self) -> usize {
        self.start + self.len
    }

    fn overlaps_with(&self, start: usize, end: usize) -> bool {
        start < self.end() && self.start < end
    }

    fn overlapping_len(&self, start: usize, end: usize) -> usize {
        let min_end = min(self.end(), end);
        let max_start = max(self.start, start);
        min_end.saturating_sub(max_start)
    }
}

pub struct Mutable {
    mappings: Vec<Mapping>,
    brk: Brk,
}

impl Mutable {
    /// Splits a mapping at `addr` into two mappings.
    fn split_at(&mut self, addr: usize) {
        // Find the mapping containing `addr`.
        let Some(left) = self.mappings.iter_mut().find(|m| {
            // Avoid "<=" intentionally to prevent creating empty mappings.
            m.start < addr && addr < m.end()
        }) else {
            return;
        };

        let file = if let Some(file) = &left.file {
            Some(BackingFile {
                offset: file.offset + (addr - left.start),
                offset_end: file.offset_end,
                file: file.file.clone(),
            })
        } else {
            None
        };

        // New mapping.
        let right = Mapping {
            start: addr,
            len: left.end() - addr,
            attrs: left.attrs,
            vmo: left.vmo.clone(),
            offset: left.offset + (addr - left.start),
            file,
        };

        // Shrink the range of the original mapping.
        left.len = addr - left.start;

        // Insert the new one. The order of mappings does not matter.
        self.mappings.push(right);
    }
}

/// A virtual memory space.
pub struct Vm {
    vmspace: VmSpace,
    mutable: SpinLock<Mutable>,
}

impl Vm {
    pub fn create(
        root_vmspace: &VmSpace,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<(Vm, usize, usize), Errno> {
        let vmspace = root_vmspace.try_clone()?;

        let mut mappings = Vec::new();
        let elf = load_elf(&vmspace, &elf_file, &mut mappings)?;

        // Find the end of the program segments.
        // TODO: Can we guarantee that mappings is not empty?
        let brk_start = mappings
            .iter()
            .map(|mapping| mapping.start + mapping.len)
            .max()
            .unwrap_or(0);
        let brk_end = BRK_END;

        let stack = Vmo::create(STACK_SIZE)?;
        let sp = prepare_stack(&stack, STACK_BOTTOM, STACK_SIZE, argv, &elf)?;

        let attrs = PageAttrs::READ | PageAttrs::WRITE;
        vmspace.map(&stack, STACK_BOTTOM, 0, STACK_SIZE, attrs)?;
        let mapping = Mapping::new(STACK_BOTTOM, STACK_SIZE, attrs, stack);
        mappings.push(mapping);

        let vm = Vm {
            vmspace,
            mutable: SpinLock::new(Mutable {
                mappings,
                brk: Brk::new(brk_start, brk_end),
            }),
        };

        Ok((vm, elf.entry, sp))
    }

    pub fn vmspace(&self) -> &VmSpace {
        &self.vmspace
    }

    pub fn mmap_anonymous(&self, _addr: usize, len: usize, prot: c_int) -> Result<usize, Errno> {
        let len = align_up(len, PAGE_SIZE);
        let attrs = attrs_from_prot(prot);

        // Allocate a VMO.
        let vmo = Vmo::create(len)?;

        // Find a space to map the new region. This must be done while holding
        // the lock to prevent other threads from picking the same address.
        let mut mutable = self.mutable.lock();
        let mut uaddr = MMAP_START;
        'retry: loop {
            // Check if the new mapping overlaps with LX's memory area.
            let end = uaddr.checked_add(len).ok_or(Errno::ENOMEM)?;
            if end > USER_END {
                return Err(Errno::ENOMEM);
            }

            for mapping in &mutable.mappings {
                if mapping.overlaps_with(uaddr, end) {
                    // The range is already mapped. Try the next hole.
                    uaddr = mapping.end();
                    continue 'retry;
                }
            }

            break;
        }

        self.vmspace.map(&vmo, uaddr, 0, len, attrs)?;
        let mapping = Mapping::new(uaddr, len, attrs, vmo);
        mutable.mappings.push(mapping);

        Ok(uaddr)
    }

    pub fn munmap(&self, addr: usize, len: usize) -> Result<(), Errno> {
        let end = validate_mapping_range(addr, len)?;
        let mut mutable = self.mutable.lock();
        self.vmspace.unmap(addr, end - addr)?;

        // Update our own mapping state.
        mutable.split_at(addr);
        mutable.split_at(end);
        mutable.mappings.retain(|m| !m.overlaps_with(addr, end));
        Ok(())
    }

    pub fn mprotect(&self, addr: usize, len: usize, prot: c_int) -> Result<(), Errno> {
        let attrs = attrs_from_prot(prot);
        let end = validate_mapping_range(addr, len)?;

        if prot & !(PROT_READ | PROT_WRITE | PROT_EXEC) != 0 {
            return Err(Errno::EINVAL);
        }

        let mut mutable = self.mutable.lock();

        // Check if the range is fully mapped.
        let mut aligned_len = end - addr;
        for mapping in &mutable.mappings {
            aligned_len -= mapping.overlapping_len(addr, end);
        }

        // Return ENOMEM if the range contains unmapped pages.
        if aligned_len != 0 {
            return Err(Errno::ENOMEM);
        }

        self.vmspace.permit(addr, end - addr, attrs)?;

        // Update our own mapping state.
        mutable.split_at(addr);
        mutable.split_at(end);
        for mapping in &mut mutable.mappings {
            if mapping.overlaps_with(addr, end) {
                mapping.attrs = attrs;
            }
        }

        Ok(())
    }

    pub fn handle_page_fault(&self, addr: usize, info: PageFaultInfo) -> Result<(), Errno> {
        if info.reason() != ErrorCode::PageAbsent {
            return Err(Errno::EFAULT);
        }

        let mutable = self.mutable.lock();

        // Find the mapping containing addr.
        let mapping = mutable
            .mappings
            .iter()
            .find(|m| m.start <= addr && addr < m.end())
            .ok_or(Errno::EFAULT)?;

        // Check if the access is allowed.
        if !mapping.attrs.contains(info.access()) {
            return Err(Errno::EFAULT);
        }

        let aligned_addr = align_down(addr, PAGE_SIZE);
        let Some(filler) = mapping.page_filler(aligned_addr) else {
            // The page fault occurred in an anonymous VMO, which means kernel
            // failed to fill the page somehow. We can't resolve this case.
            return Err(Errno::EFAULT);
        };

        drop(mutable);
        filler.fill()?;
        Ok(())
    }

    /// Writes `buf` to the user memory at `addr`.
    ///
    /// This is useful when you want to write to another process's memory.
    pub fn write(&self, addr: usize, buf: &[u8]) -> Result<(), Errno> {
        let end = addr.checked_add(buf.len()).ok_or(Errno::EFAULT)?;

        // Find the mapping containing `addr`.
        let mutable = self.mutable.lock();
        let mapping = mutable
            .mappings
            .iter()
            .find(|m| m.start <= addr && end <= m.end())
            .ok_or(Errno::EFAULT)?;

        if !mapping.attrs.contains(PageAttrs::WRITE) {
            return Err(Errno::EFAULT);
        }

        let vmo_offset = mapping.offset + (addr - mapping.start);
        loop {
            match mapping.vmo.write(vmo_offset, buf) {
                Ok(()) => {
                    return Ok(());
                }
                Err(ErrorCode::PageAbsent) => {
                    // Handle a page fault, and try again.
                    let aligned_addr = align_down(addr, PAGE_SIZE);
                    for page in (aligned_addr..end).step_by(PAGE_SIZE) {
                        if let Some(filler) = mapping.page_filler(page) {
                            filler.fill()?;
                        }
                    }
                }
                Err(error) => {
                    return Err(error.into());
                }
            }
        }
    }

    /// `brk(2)` system call.
    ///
    /// Returns the new break address, even if it fails. This is a documented
    /// behavior of Linux:
    ///
    /// > On failure, the system call returns the current break.
    /// >
    /// > https://man7.org/linux/man-pages/man2/brk.2.html
    pub fn brk(&self, addr: usize) -> usize {
        let mut mutable = self.mutable.lock();
        let brk = mutable.brk;
        if addr < brk.start || addr > brk.end {
            return brk.current;
        }

        // Align the address to the page boundary.
        let current_aligned = align_up(addr, PAGE_SIZE);
        if current_aligned <= brk.current_aligned {
            // The page is already allocated. Advance the break address and
            // return immediately.
            mutable.brk.current = addr;
            return addr;
        }

        // We'll do system calls might schedule to another thread in the same
        // process. Release the lock.
        drop(mutable);

        // Allocate pages for the new area.
        let len = current_aligned - brk.current_aligned;
        let attrs = PageAttrs::READ | PageAttrs::WRITE;
        let Ok(vmo) = Vmo::create(len) else {
            return brk.current;
        };

        // Map the pages.
        if self
            .vmspace
            .map(&vmo, brk.current_aligned, 0, len, attrs)
            .is_err()
        {
            // This may fail if there are concurrent brk calls, and we've lost
            // the race. Return the latest break address.
            let mutable = self.mutable.lock();
            return mutable.brk.current;
        }

        // Record the mapping.
        let mut mutable = self.mutable.lock();
        let mapping = Mapping::new(brk.current_aligned, len, attrs, vmo);
        mutable.mappings.push(mapping);

        mutable.brk.current = addr;
        mutable.brk.current_aligned = current_aligned;
        addr
    }

    pub fn fork(&self, root_vmspace: &VmSpace) -> Result<Self, Errno> {
        let vmspace = root_vmspace.try_clone()?;

        let mutable = self.mutable.lock();
        let brk = mutable.brk;
        let mut mappings = Vec::with_capacity(mutable.mappings.len());
        for mapping in &mutable.mappings {
            let new_vmo = mapping.vmo.snapshot(mapping.offset, mapping.len)?;
            vmspace.map(&new_vmo, mapping.start, 0, mapping.len, mapping.attrs)?;
            mappings.push(Mapping {
                vmo: Arc::new(new_vmo),
                offset: 0,
                ..mapping.clone()
            });
        }

        Ok(Self {
            vmspace,
            mutable: SpinLock::new(Mutable { mappings, brk }),
        })
    }
}

/// An owned file-to-vmo transfer.
///
/// This struct extracts things from mappings that we need to read and fill a
/// file-backed mapping, so that we don't need to hold the Vm lock.
struct PageFiller {
    vmo: Arc<Vmo>,
    file: Arc<dyn FileLike>,
    file_offset: usize,
    vmo_offset: usize,
    len: usize,
}

impl PageFiller {
    pub fn fill(self) -> Result<(), Errno> {
        // Read the page from the file.
        // TODO: Zero-filling the vec is not necessary.
        // TODO: Eliminate this memory copy.
        let mut buf = vec![0u8; PAGE_SIZE];
        read_exact(self.file.as_ref(), self.file_offset, &mut buf[..self.len])?;

        // Fill the page with the file data.
        self.vmo.supply(self.vmo_offset, &buf)?;
        Ok(())
    }
}

/// Returns `addr + len`. Returns an error if they are not valid parameters.
///
/// Note: `len` might not be aligned to `PAGE_SIZE`.
fn validate_mapping_range(addr: usize, len: usize) -> Result<usize, Errno> {
    if len == 0 || !is_aligned(addr, PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }

    let Some(aligned_len) = len.checked_next_multiple_of(PAGE_SIZE) else {
        return Err(Errno::EINVAL);
    };

    let Some(end) = addr.checked_add(aligned_len) else {
        return Err(Errno::EINVAL);
    };

    if end > USER_END {
        return Err(Errno::EINVAL);
    }

    Ok(end)
}

fn attrs_from_prot(prot: c_int) -> PageAttrs {
    let mut attrs = PageAttrs::EMPTY;
    if prot & PROT_EXEC != 0 {
        attrs |= PageAttrs::EXEC;
    }

    if prot & PROT_WRITE != 0 {
        attrs |= PageAttrs::WRITE;
    }

    if prot & PROT_READ != 0 {
        attrs |= PageAttrs::READ;
    }

    attrs
}

fn attrs_from_phdr(phdr: &ftl_elf::Phdr) -> PageAttrs {
    let mut attrs = PageAttrs::EMPTY;
    if phdr.p_flags & PF_X != 0 {
        attrs |= PageAttrs::EXEC;
    }

    if phdr.p_flags & PF_W != 0 {
        attrs |= PageAttrs::WRITE;
    }

    if phdr.p_flags & PF_R != 0 {
        attrs |= PageAttrs::READ;
    }

    attrs
}

fn read_exact(file: &dyn FileLike, mut offset: usize, buf: &mut [u8]) -> Result<(), Errno> {
    let mut total = 0;
    while total < buf.len() {
        let n = file.read(&mut buf[total..], offset, false, Sleep::Uninterruptible)?;
        if n == 0 {
            return Err(Errno::ENOEXEC);
        }

        total += n;
        offset += n;
    }

    Ok(())
}

fn read_uninit<T: Copy>(
    file: &dyn FileLike,
    offset: usize,
    buf: &mut MaybeUninit<T>,
) -> Result<T, Errno> {
    let slice = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, size_of::<T>()) };
    read_exact(file, offset, slice)?;
    // SAFETY: read_exact guarantees that the buffer is filled.
    Ok(unsafe { buf.assume_init() })
}

struct LoadedElf {
    entry: usize,
    phdr: usize,
    phent: usize,
    phnum: usize,
}

fn load_elf(
    vmspace: &VmSpace,
    elf_file: &Arc<dyn FileLike>,
    mappings: &mut Vec<Mapping>,
) -> Result<LoadedElf, Errno> {
    let mut ehdr = MaybeUninit::<ftl_elf::Ehdr>::uninit();
    let ehdr = read_uninit(elf_file.as_ref(), 0, &mut ehdr)?;

    let phdrs_end = ehdr.e_phoff as usize + ehdr.e_phnum as usize * size_of::<ftl_elf::Phdr>();
    let mut header_region = vec![0u8; phdrs_end]; // TODO: Use MaybeUninit
    read_exact(elf_file.as_ref(), 0, &mut header_region)?;

    let elf = Elf::parse(&header_region, ftl_elf::ET_EXEC).map_err(|_| Errno::ENOEXEC)?;
    let file_size = elf_file.size().map_err(|_| Errno::ENOEXEC)?;
    let mut phdr_vaddr = 0;
    for phdr in elf.phdrs {
        if phdr.p_type == PhdrType::Phdr as u32 {
            phdr_vaddr = phdr.p_vaddr as usize;
        }

        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let region_base = align_down(vaddr, PAGE_SIZE);
        let page_offset = vaddr - region_base;
        let region_len = align_up(page_offset + phdr.p_memsz as usize, PAGE_SIZE);
        let file_len = align_up(page_offset + phdr.p_filesz as usize, PAGE_SIZE);

        // Calculate the offset in the file. Subtract page_offset since the
        // mapping starts at a page boundary (region_base), not p_vaddr.
        let file_offset = (phdr.p_offset as usize)
            .checked_sub(page_offset)
            .ok_or(Errno::ENOEXEC)?;

        let Some(file_offset_end) = (phdr.p_offset as usize).checked_add(phdr.p_filesz as usize)
        else {
            return Err(Errno::ENOEXEC);
        };

        if file_offset_end > file_size {
            return Err(Errno::ENOEXEC);
        }

        if phdr.p_filesz > phdr.p_memsz {
            return Err(Errno::ENOEXEC);
        }

        let attrs = attrs_from_phdr(phdr);
        if file_len > 0 {
            let vmo = Vmo::create_user(file_len)?;
            vmspace.map(&vmo, region_base, 0, file_len, attrs)?;
            let mut mapping = Mapping::new(region_base, file_len, attrs, vmo);
            mapping.file = Some(BackingFile {
                file: elf_file.clone(),
                offset: file_offset,
                offset_end: file_offset_end,
            });
            mappings.push(mapping);
        }

        if region_len > file_len {
            // If p_memsz > p_filesz, allocate an anonymous VMO to provide
            // zero-filled pages by kernel, not via user page faults.
            let start = region_base + file_len;
            let size = region_len - file_len;
            let vmo = Vmo::create(size)?;
            vmspace.map(&vmo, start, 0, size, attrs)?;
            mappings.push(Mapping::new(start, size, attrs, vmo));
        }
    }

    Ok(LoadedElf {
        entry: elf.ehdr.e_entry as usize,
        phdr: phdr_vaddr,
        phent: elf.ehdr.e_phentsize as usize,
        phnum: elf.ehdr.e_phnum as usize,
    })
}

fn prepare_stack(
    stack: &Vmo,
    sp_bottom: usize,
    stack_size: usize,
    argv: &[&[u8]],
    elf: &LoadedElf,
) -> Result<usize, Errno> {
    // FIXME: Reject too long argv / envp / auxv.
    let mut words = Vec::new();

    // argc
    words.push(argv.len());

    // argv
    let strings_len: usize = argv.iter().map(|arg| arg.len() + 1).sum();
    let args_offset = stack_size
        .checked_sub(strings_len + AT_RANDOM_LEN)
        .ok_or(Errno::ENOMEM)?;
    let mut offset = args_offset;
    for arg in argv {
        stack.write(offset, arg)?;
        words.push(sp_bottom + offset);
        offset += arg.len();
        stack.write(offset, &[0])?;
        offset += 1;
    }
    words.push(0); // NULL (terminator)

    // TODO: envp
    words.push(0); // NULL (terminator)

    // Read random bytes for AT_RANDOM.
    let mut random = [0u8; AT_RANDOM_LEN];
    ftl::random::read(&mut random)?;
    stack.write(offset, &random)?;
    let random_addr = sp_bottom + offset;

    // auxv
    words.extend([AT_PHDR, elf.phdr]);
    words.extend([AT_PHENT, elf.phent]);
    words.extend([AT_PHNUM, elf.phnum]);
    words.extend([AT_PAGESZ, PAGE_SIZE]);
    words.extend([AT_RANDOM, random_addr]);
    words.extend([0, 0]); // AT_NULL

    // Align to 16 bytes (x64 ABI requirement).
    let len = words.len() * size_of::<usize>();
    let sp_offset = align_down(args_offset.checked_sub(len).ok_or(Errno::ENOMEM)?, 16);

    // Copy argc, argv/envp pointers, and auxv.
    let bytes = unsafe { slice::from_raw_parts(words.as_ptr().cast(), len) };
    stack.write(sp_offset, bytes)?;
    Ok(sp_bottom + sp_offset)
}

/// Reads a byte in each page of the buffer to trigger page faults, to handle
/// page faults proactively.
///
/// Call this when a system call fails with `ErrorCode::PageAbsent`. Kernel
/// returns it when memory access fails due to absent pages, not bad access like
/// writes to read-only pages.
pub fn trigger_proactive_page_faults(buf: &[u8]) {
    let range = buf.as_ptr_range();
    let end = range.end as usize;
    let mut addr = range.start as usize;

    // Read a byte in each page.
    while addr < end {
        // SAFETY: addr is in the buffer.
        unsafe { (addr as *const u8).read_volatile() };

        // Move to the beginning of the next page.
        addr = align_down(addr, PAGE_SIZE) + PAGE_SIZE;
    }
}
