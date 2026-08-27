use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::mem::MaybeUninit;
use core::slice;

use ftl::syscall::poll_create;
use ftl::syscall::poll_notify;
use ftl::syscall::poll_wait;
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
use crate::container::Container;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::FileLike;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_BOTTOM: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PId(c_int);

impl PId {
    pub const fn new(id: c_int) -> Self {
        Self(id)
    }
}

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
    parent: Option<Weak<Process>>,
    threads: Vec<Arc<Thread>>,
    mappings: Vec<Mapping>,
    children: Vec<Arc<Process>>,
    exit_status: Option<c_int>,
}

pub struct Process {
    tgid: PId,
    poll: HandleId,
    container: Arc<Container>,
    mutable: SpinLock<Mutable>,
    fd_table: SpinLock<FdTable>,
}

impl Process {
    pub fn new_init(
        container: Arc<Container>,
        elf_file: Arc<dyn FileLike>,
    ) -> Result<Arc<Process>, Errno> {
        let (vmspace, mappings, entry, sp) =
            create_address_space(container.root_vmspace, elf_file, &[])?;

        let mut fd_table = FdTable::new(1024); // TODO: make this configurable
        let console: Arc<dyn FileLike> = Arc::new(Console::new());
        fd_table.insert_at(0, console.clone())?;
        fd_table.insert_at(1, console.clone())?;
        fd_table.insert_at(2, console)?;

        let process = Self::new(
            container,
            vmspace,
            fd_table,
            PId(1),
            None,
            mappings,
            entry,
            sp,
            |_thread| Ok(()),
        )?;

        Ok(process)
    }

    pub fn exec(
        self: &Arc<Self>,
        current: &Thread,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<(), Errno> {
        let (vmspace, mappings, entry, sp) =
            create_address_space(self.container.root_vmspace, elf_file, argv)?;
        let thread = Thread::new(
            self.container.isolate,
            vmspace,
            entry,
            sp,
            Arc::downgrade(self),
            self.tgid,
        )?;
        thread.start()?;

        let mut mutable = self.mutable.lock();
        mutable.mappings = mappings;
        mutable
            .threads
            .retain(|thread| !core::ptr::eq(thread.as_ref(), current));
        mutable.threads.push(thread);
        Ok(())
    }

    fn new<F>(
        container: Arc<Container>,
        vmspace: HandleId,
        fd_table: FdTable,
        tgid: PId,
        parent: Option<Weak<Process>>,
        mappings: Vec<Mapping>,
        entry: usize,
        sp: usize,
        thread_prestart: F,
    ) -> Result<Arc<Self>, Errno>
    where
        F: FnOnce(&Arc<Thread>) -> Result<(), Errno>,
    {
        let poll = poll_create()?;
        let process = Arc::new(Self {
            tgid,
            poll,
            container: container.clone(),
            mutable: SpinLock::new(Mutable {
                parent,
                threads: Vec::with_capacity(1),
                mappings,
                children: Vec::new(),
                exit_status: None,
            }),
            fd_table: SpinLock::new(fd_table),
        });

        // TODO: LX assumes that the cookie won't be dereferenced until the
        // thread is started. Should we document and guarantee this?
        let thread = Thread::new(
            container.isolate,
            vmspace,
            entry,
            sp,
            Arc::downgrade(&process),
            tgid,
        )?;
        thread_prestart(&thread)?;

        // Start the thread.
        process.mutable.lock().threads.push(thread.clone());
        thread.start()?;

        Ok(process)
    }

    pub fn fork(self: &Arc<Self>, current: &Thread, syscall_sp: usize) -> Result<PId, Errno> {
        let vmspace = vmspace_clone(self.container.root_vmspace)?;
        let fd_table = self.fd_table.lock().clone();

        // Copy memory into the child's VM space.
        // TODO: copy on write
        let mappings = self.mutable.lock().mappings.clone();
        for mapping in &mappings {
            let vmo = vmo_create(mapping.len)?;
            let bytes =
                unsafe { core::slice::from_raw_parts(mapping.start as *const u8, mapping.len) };
            vmo_write(vmo, 0, bytes)?;
            vmspace_map(vmspace, vmo, mapping.start, mapping.attrs)?;
        }

        // Allocate a new PID for the child process.
        let mut pid_table = self.container.processes.lock();
        let tgid = pid_table.allocate()?;

        // Create a new process and the first thread.
        let entry = fork_child_entry as *const () as usize;
        let child = Self::new(
            self.container.clone(),
            vmspace,
            fd_table,
            tgid,
            Some(Arc::downgrade(self)),
            mappings,
            entry,
            syscall_sp,
            |thread| {
                current.copy_regs_to(&thread, RegsKind::FsBase)?;
                current.copy_regs_to(&thread, RegsKind::FpAndVector)?;
                Ok(())
            },
        )?;

        pid_table.insert(tgid, child.clone());
        self.mutable.lock().children.push(child);
        Ok(tgid)
    }

    // TODO: Should we make this method infallible?
    pub fn exit(&self, status: c_int) -> Result<(), Errno> {
        if self.tgid == PId::new(1) {
            panic!("init process exited with status {}", status);
        }

        let mut mutable = self.mutable.lock();

        // Wake up the parent process while holding the lock. When poll_notify
        // fails, exit fails and keeps this process alive.
        let parent = mutable.parent.as_ref().and_then(Weak::upgrade);
        if let Some(parent) = &parent {
            poll_notify(parent.poll)?;
        }

        // Mark the process as exited.
        mutable.exit_status = Some(status);

        // Reap this process if its parent is gone.
        if parent.is_none() {
            self.container.processes.lock().remove(self.tgid);
        }

        // Orphan its children that have already exited.
        while let Some(child) = mutable.children.pop() {
            let mut child_mutable = child.mutable.lock();

            // Drop the reference to this process. It has ceased to be. It is an ex-process.
            child_mutable.parent = None;

            if child_mutable.exit_status.is_some() {
                self.container.processes.lock().remove(child.tgid);
            }
        }
        Ok(())
    }

    pub fn wait(&self, pid: c_int) -> Result<(PId, c_int), Errno> {
        if pid != -1 && pid <= 0 {
            return Err(Errno::EINVAL);
        }

        loop {
            let mut mutable = self.mutable.lock();
            let mut matched_any = false;
            for (index, child) in mutable.children.iter().enumerate() {
                if pid != -1 && child.tgid != PId::new(pid) {
                    // This child is not the one we are waiting for.
                    continue;
                }

                let exit_status = child.mutable.lock().exit_status;
                if let Some(status) = exit_status {
                    let tgid = child.tgid;
                    mutable.children.remove(index);
                    self.container.processes.lock().remove(tgid);
                    return Ok((tgid, status));
                }

                matched_any = true;
            }

            if !matched_any {
                return Err(Errno::ECHILD);
            }

            poll_wait(self.poll)?;
        }
    }

    pub fn fd_table(&self) -> &SpinLock<FdTable> {
        &self.fd_table
    }
}

fn prepare_stack(
    stack: HandleId,
    sp_bottom: usize,
    stack_size: usize,
    argv: &[&[u8]],
) -> Result<usize, Errno> {
    // FIXME: Reject too long argv / envp / auxv.
    let mut words = Vec::new();

    // argc
    words.push(argv.len());

    // argv
    let strings_len: usize = argv.iter().map(|arg| arg.len()).sum();
    let args_offset = stack_size - strings_len;
    let mut offset = args_offset;
    for arg in argv {
        vmo_write(stack, offset, arg)?;
        words.push(sp_bottom + offset);
        offset += arg.len();
    }
    words.push(0); // NULL (terminator)

    // TODO: envp
    words.push(0); // NULL (terminator)

    // auxv
    words.extend([0, 0]); // AT_NULL

    // Align to 16 bytes (x64 ABI requirement).
    let len = words.len() * size_of::<usize>();
    let sp_offset = align_down(args_offset - len, 16);

    // Copy argc, argv/envp pointers, and auxv.
    let bytes = unsafe { slice::from_raw_parts(words.as_ptr().cast(), len) };
    vmo_write(stack, sp_offset, bytes)?;
    Ok(sp_bottom + sp_offset)
}

fn create_address_space(
    root_vmspace: HandleId,
    elf_file: Arc<dyn FileLike>,
    argv: &[&[u8]],
) -> Result<(HandleId, Vec<Mapping>, usize, usize), Errno> {
    let vmspace = vmspace_clone(root_vmspace)?;
    let mut mappings = Vec::new();
    let entry = load_elf(vmspace, elf_file.as_ref(), &mut mappings)?;

    let stack = vmo_create(STACK_SIZE)?;
    let sp = prepare_stack(stack, STACK_BOTTOM, STACK_SIZE, argv)?;

    vmspace_map(
        vmspace,
        stack,
        STACK_BOTTOM,
        PageAttrs::READ | PageAttrs::WRITE,
    )?;
    mappings.push(Mapping {
        start: STACK_BOTTOM,
        len: STACK_SIZE,
        attrs: PageAttrs::READ | PageAttrs::WRITE,
    });

    Ok((vmspace, mappings, entry, sp))
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
