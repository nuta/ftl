use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
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
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
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

pub(crate) const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_BOTTOM: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;
const BRK_END: usize = STACK_BOTTOM;

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

#[derive(Clone, Copy)]
struct Mapping {
    start: usize,
    len: usize,
    attrs: PageAttrs,
}

pub struct Mutable {
    mappings: Vec<Mapping>,
    brk: Brk,
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
        let elf = load_elf(&vmspace, elf_file.as_ref(), &mut mappings)?;

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

        vmspace.map(&stack, STACK_BOTTOM, PageAttrs::READ | PageAttrs::WRITE)?;
        mappings.push(Mapping {
            start: STACK_BOTTOM,
            len: STACK_SIZE,
            attrs: PageAttrs::READ | PageAttrs::WRITE,
        });

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

        // Find a space to map the new region.
        let uaddr = {
            let mutable = self.mutable.lock();
            // TODO: Better way to find a hole in mappings.
            mutable
                .mappings
                .iter()
                .map(|mapping| mapping.start + mapping.len)
                .max()
                .unwrap_or(0)
        };

        // Allocate a VMO and map it.
        let vmo = Vmo::create(len)?;

        let mut mutable = self.mutable.lock();
        self.vmspace.map(&vmo, uaddr, attrs)?;
        mutable.mappings.push(Mapping {
            start: uaddr,
            len,
            attrs,
        });

        Ok(uaddr)
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
        if self.vmspace.map(&vmo, brk.current_aligned, attrs).is_err() {
            // This may fail if there are concurrent brk calls, and we've lost
            // the race. Return the latest break address.
            let mutable = self.mutable.lock();
            return mutable.brk.current;
        }

        // Record the mapping.
        let mut mutable = self.mutable.lock();
        mutable.mappings.push(Mapping {
            start: brk.current_aligned,
            len,
            attrs,
        });

        mutable.brk.current = addr;
        mutable.brk.current_aligned = current_aligned;
        addr
    }

    pub fn fork(&self, root_vmspace: &VmSpace) -> Result<Self, Errno> {
        let vmspace = root_vmspace.try_clone()?;

        let mutable = self.mutable.lock();
        let mappings = mutable.mappings.clone();
        let brk = mutable.brk;
        for mapping in &mappings {
            let vmo = Vmo::create(mapping.len)?;
            let bytes =
                unsafe { core::slice::from_raw_parts(mapping.start as *const u8, mapping.len) };
            vmo.write(0, bytes)?;
            vmspace.map(&vmo, mapping.start, mapping.attrs)?;
        }

        Ok(Self {
            vmspace,
            mutable: SpinLock::new(Mutable { mappings, brk }),
        })
    }
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
        let n = file.read(&mut buf[total..], offset, false)?;
        assert!(n > 0); // FIXME: proper errno
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
    elf_file: &dyn FileLike,
    mappings: &mut Vec<Mapping>,
) -> Result<LoadedElf, Errno> {
    let mut ehdr = MaybeUninit::<ftl_elf::Ehdr>::uninit();
    let ehdr = read_uninit(elf_file, 0, &mut ehdr)?;

    let phdrs_end = ehdr.e_phoff as usize + ehdr.e_phnum as usize * size_of::<ftl_elf::Phdr>();
    let mut header_region = vec![0u8; phdrs_end]; // TODO: Use MaybeUninit
    read_exact(elf_file, 0, &mut header_region)?;

    let elf = Elf::parse(&header_region, ftl_elf::ET_EXEC).expect("failed to parse ELF");
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
        let vmo = Vmo::create(region_len).unwrap();

        let filesz = phdr.p_filesz as usize;
        let mut buf = [0u8; PAGE_SIZE];
        let mut offset = 0;
        while offset < filesz {
            let len = min(buf.len(), filesz - offset);
            let chunk = &mut buf[..len];
            // FIXME: do not copy twice
            read_exact(elf_file, phdr.p_offset as usize + offset, chunk)?;
            vmo.write(page_offset + offset, chunk)?;
            offset += len;
        }

        let attrs = attrs_from_phdr(phdr);
        vmspace.map(&vmo, region_base, attrs)?;
        mappings.push(Mapping {
            start: region_base,
            len: region_len,
            attrs,
        });
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
