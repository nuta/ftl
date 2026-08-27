use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::mem::MaybeUninit;
use core::slice;
use core::sync::atomic::AtomicI32;
use core::sync::atomic::Ordering;

use ftl::syscall::vmo_create;
use ftl::syscall::vmo_write;
use ftl::syscall::vmspace_clone;
use ftl::syscall::vmspace_map;
use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::handle::HandleId;
use ftl_types::thread::RegsKind;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::arch::fork_child_entry;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::FileLike;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_START: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;
static NEXT_PID: AtomicI32 = AtomicI32::new(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PId(c_int);

impl fmt::Display for PId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PId {
    pub fn as_int(self) -> c_int {
        self.0
    }
}

#[derive(Clone, Copy)]
struct Mapping {
    start: usize,
    len: usize,
    attrs: PageAttrs,
}

#[derive(Clone)]
pub struct FdTable {
    open_files: Vec<Option<Arc<dyn FileLike>>>,
    active_fds: usize,
    capacity: usize,
}

impl FdTable {
    pub fn new(capacity: usize) -> Self {
        Self {
            open_files: Vec::new(),
            active_fds: 0,
            capacity,
        }
    }

    pub fn insert(&mut self, file: Arc<dyn FileLike>) -> Result<Option<Arc<dyn FileLike>>, Errno> {
        if self.active_fds >= self.capacity {
            return Err(Errno::EMFILE);
        }

        for fd in 0..self.capacity {
            if fd >= self.open_files.len() || self.open_files[fd].is_none() {
                return self.insert_at(fd as c_int, file);
            }
        }

        Err(Errno::EMFILE)
    }

    pub fn insert_at(
        &mut self,
        fd: c_int,
        file: Arc<dyn FileLike>,
    ) -> Result<Option<Arc<dyn FileLike>>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let fd = fd as usize;
        if fd >= self.capacity {
            return Err(Errno::EMFILE);
        }

        if fd >= self.open_files.len() {
            self.open_files.resize(fd + 1, None);
        }

        let old = self.open_files[fd].replace(file);
        if old.is_none() {
            self.active_fds += 1;
        }
        Ok(old)
    }

    pub fn get(&self, fd: c_int) -> Result<&Arc<dyn FileLike>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let slot = self.open_files.get(fd as usize);
        match slot {
            Some(Some(file)) => Ok(file),
            _ => Err(Errno::EBADF),
        }
    }
}

struct Mutable {
    threads: Vec<Arc<Thread>>,
}

pub struct Process {
    tgid: PId,
    isolate: HandleId,
    root_vmspace: HandleId,
    mappings: Vec<Mapping>,
    mutable: SpinLock<Mutable>,
    fd_table: SpinLock<FdTable>,
}

impl Process {
    pub fn new_init(
        root_isolate: HandleId,
        root_vmspace: HandleId,
        elf_file: Arc<dyn FileLike>,
    ) -> Result<Arc<Process>, Errno> {
        let vmspace = vmspace_clone(root_vmspace)?;
        let mut mappings = Vec::new();
        let entry = load_elf(vmspace, elf_file.as_ref(), &mut mappings)?;
        let stack = vmo_create(STACK_SIZE)?;
        vmspace_map(
            vmspace,
            stack,
            STACK_START,
            PageAttrs::READ | PageAttrs::WRITE,
        )?;
        mappings.push(Mapping {
            start: STACK_START,
            len: STACK_SIZE,
            attrs: PageAttrs::READ | PageAttrs::WRITE,
        });

        // TODO: implement argc, argv, envp, auxv
        let sp = align_down(STACK_START + STACK_SIZE - 5 * size_of::<usize>(), 16);

        let mut fd_table = FdTable::new(1024); // TODO: make this configurable
        let console: Arc<dyn FileLike> = Arc::new(Console::new());
        fd_table.insert_at(0, console.clone())?;
        fd_table.insert_at(1, console.clone())?;
        fd_table.insert_at(2, console)?;

        let process = Self::new(
            root_isolate,
            root_vmspace,
            vmspace,
            fd_table,
            PId(1),
            mappings,
            entry,
            sp,
            |_thread| Ok(()),
        )?;

        Ok(process)
    }

    fn new<F>(
        isolate: HandleId,
        root_vmspace: HandleId,
        vmspace: HandleId,
        fd_table: FdTable,
        tgid: PId,
        mappings: Vec<Mapping>,
        entry: usize,
        sp: usize,
        thread_prestart: F,
    ) -> Result<Arc<Self>, Errno>
    where
        F: FnOnce(&Arc<Thread>) -> Result<(), Errno>,
    {
        let process = Arc::new(Self {
            tgid,
            isolate,
            root_vmspace,
            mappings,
            mutable: SpinLock::new(Mutable {
                threads: Vec::with_capacity(1),
            }),
            fd_table: SpinLock::new(fd_table),
        });

        // TODO: LX assumes that the cookie won't be dereferenced until the
        // thread is started. Should we document and guarantee this?
        let thread = Thread::new(isolate, vmspace, entry, sp, Arc::downgrade(&process), tgid)?;
        thread_prestart(&thread)?;

        // Start the thread.
        process.mutable.lock().threads.push(thread.clone());
        thread.start()?;

        Ok(process)
    }

    pub fn fork(self: &Arc<Self>, current: &Thread, syscall_sp: usize) -> Result<PId, Errno> {
        let vmspace = vmspace_clone(self.root_vmspace)?;
        let fd_table = self.fd_table.lock().clone();

        // Copy memory into the child's VM space.
        // TODO: copy on write
        for mapping in &self.mappings {
            let vmo = vmo_create(mapping.len)?;
            let bytes =
                unsafe { core::slice::from_raw_parts(mapping.start as *const u8, mapping.len) };
            vmo_write(vmo, 0, bytes)?;
            vmspace_map(vmspace, vmo, mapping.start, mapping.attrs)?;
        }

        // FIXME: PID table to check conflicts
        let tgid = PId(NEXT_PID.fetch_add(1, Ordering::Relaxed));

        // Create a new process and the first thread.
        let entry = fork_child_entry as *const () as usize;
        let child = Self::new(
            self.isolate,
            self.root_vmspace,
            vmspace,
            fd_table,
            tgid,
            self.mappings.clone(),
            entry,
            syscall_sp,
            |thread| {
                current.copy_regs_to(&thread, RegsKind::FsBase)?;
                current.copy_regs_to(&thread, RegsKind::FpAndVector)?;
                Ok(())
            },
        )?;

        // FIXME: PID table to keep the ref count
        core::mem::forget(child);

        Ok(tgid)
    }

    pub fn fd_table(&self) -> &SpinLock<FdTable> {
        &self.fd_table
    }
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
        let n = file.read(&mut buf[total..], offset)?;
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

fn load_elf(
    vmspace: HandleId,
    elf_file: &dyn FileLike,
    mappings: &mut Vec<Mapping>,
) -> Result<usize, Errno> {
    let mut ehdr = MaybeUninit::<ftl_elf::Ehdr>::uninit();
    let ehdr = read_uninit(elf_file, 0, &mut ehdr)?;

    let phdrs_end = ehdr.e_phoff as usize + ehdr.e_phnum as usize * size_of::<ftl_elf::Phdr>();
    let mut header_region = vec![0u8; phdrs_end]; // TODO: Use MaybeUninit
    read_exact(elf_file, 0, &mut header_region)?;

    let elf = Elf::parse(&header_region, ftl_elf::ET_EXEC).expect("failed to parse hello ELF");
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let region_base = align_down(vaddr, PAGE_SIZE);
        let page_offset = vaddr - region_base;
        let region_len = align_up(page_offset + phdr.p_memsz as usize, PAGE_SIZE);
        let vmo = vmo_create(region_len).unwrap();

        let filesz = phdr.p_filesz as usize;
        if filesz > 0 {
            let mut buf = vec![0u8; filesz]; // FIXME: do not copy twice
            read_exact(elf_file, phdr.p_offset as usize, &mut buf)?;
            vmo_write(vmo, page_offset, &buf)?;
        }

        let attrs = attrs_from_phdr(phdr);
        vmspace_map(vmspace, vmo, region_base, attrs)?;
        mappings.push(Mapping {
            start: region_base,
            len: region_len,
            attrs,
        });
    }

    Ok(elf.ehdr.e_entry as usize)
}
