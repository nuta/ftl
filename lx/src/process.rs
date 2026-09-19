use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::min;
use core::fmt;
use core::mem::MaybeUninit;
use core::slice;

use ftl::trace;
use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::thread::RegsKind;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::arch::SyscallFrame;
use crate::arch::restore_regs;
use crate::container::Container;
use crate::open_file::OpenFile;
use crate::signal::SigAction;
use crate::signal::SigDisposition;
use crate::signal::Signal;
use crate::signal::SignalMap;
use crate::signal::SignalSet;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::auxv::AT_PAGESZ;
use crate::types::sys::auxv::AT_PHDR;
use crate::types::sys::auxv::AT_PHENT;
use crate::types::sys::auxv::AT_PHNUM;
use crate::types::sys::auxv::AT_RANDOM;
use crate::types::sys::auxv::AT_RANDOM_LEN;
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_RDONLY;
use crate::types::sys::fcntl::O_WRONLY;
use crate::types::sys::mman::MAP_ANONYMOUS;
use crate::types::sys::mman::PROT_EXEC;
use crate::types::sys::mman::PROT_READ;
use crate::types::sys::mman::PROT_WRITE;
use crate::vfs::Console;
use crate::vfs::FileLike;
use crate::wait_queue::WaitQueue;
use crate::wait_queue::WaitSet;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_BOTTOM: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;
const BRK_END: usize = STACK_BOTTOM;

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

#[derive(Clone, Copy)]
struct Brk {
    /// The start of the heap.
    start: usize,
    /// The current break address.
    current: usize,
    /// The current break address, aligned to the page boundary.
    current_aligned: usize,
}

impl Brk {
    fn new(start: usize) -> Self {
        Self {
            start,
            current: start,
            current_aligned: start,
        }
    }
}

#[derive(Clone)]
struct Entry {
    file: Arc<OpenFile>,
    cloexec: bool,
}

#[derive(Clone)]
pub struct FdTable {
    open_files: Vec<Option<Entry>>,
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

    pub fn insert(&mut self, file: Arc<dyn FileLike>, flags: c_int) -> Result<c_int, Errno> {
        if self.active_fds >= self.capacity {
            return Err(Errno::EMFILE);
        }

        for fd in 0..self.capacity {
            if fd >= self.open_files.len() || self.open_files[fd].is_none() {
                self.insert_at(fd as c_int, file, flags)?;
                return Ok(fd as c_int);
            }
        }

        Err(Errno::EMFILE)
    }

    pub fn insert_at(
        &mut self,
        fd: c_int,
        file: Arc<dyn FileLike>,
        flags: c_int,
    ) -> Result<(), Errno> {
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

        let new = Entry {
            file: Arc::new(OpenFile::new(file, flags)),
            cloexec: flags & O_CLOEXEC != 0,
        };

        let old = self.open_files[fd].replace(new);
        if old.is_none() {
            self.active_fds += 1;
        }

        Ok(())
    }

    pub fn get(&self, fd: c_int) -> Result<&Arc<OpenFile>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let slot = self.open_files.get(fd as usize);
        match slot {
            Some(Some(entry)) => Ok(&entry.file),
            _ => Err(Errno::EBADF),
        }
    }

    /// Returns if the fd is marked as close-on-exec.
    pub fn get_cloexec(&self, fd: c_int) -> Result<bool, Errno> {
        let slot = self.open_files.get(fd as usize);
        match slot {
            Some(Some(entry)) => Ok(entry.cloexec),
            _ => Err(Errno::EBADF),
        }
    }

    /// Updates the close-on-exec flag for the fd.
    pub fn set_cloexec(&mut self, fd: c_int, cloexec: bool) -> Result<(), Errno> {
        let slot = self.open_files.get_mut(fd as usize);
        match slot {
            Some(Some(entry)) => {
                entry.cloexec = cloexec;
                Ok(())
            }
            _ => Err(Errno::EBADF),
        }
    }

    pub fn remove(&mut self, fd: c_int) -> Result<Arc<OpenFile>, Errno> {
        if fd < 0 {
            return Err(Errno::EBADF);
        }

        let slot = self.open_files.get_mut(fd as usize);
        let entry = match slot {
            Some(entry) => entry.take().ok_or(Errno::EBADF)?,
            _ => return Err(Errno::EBADF),
        };

        self.active_fds -= 1;
        Ok(entry.file)
    }

    /// Closes file descriptors that are marked close-on-exec.
    pub fn close_on_exec(&mut self) {
        for slot in &mut self.open_files {
            if let Some(entry) = slot.as_ref() {
                if entry.cloexec {
                    *slot = None;
                    self.active_fds -= 1;
                }
            }
        }
    }

    fn clear(&mut self) {
        self.active_fds = 0;
        self.open_files.clear();
    }
}

struct Mutable {
    parent: Option<Weak<Process>>,
    threads: Vec<Arc<LxThread>>,
    mappings: Vec<Mapping>,
    brk: Brk,
    children: Vec<Arc<Process>>,
    exit_status: Option<c_int>,
    signal_actions: SignalMap<SigAction>,
    pending_signals: SignalSet,
}

pub struct Process {
    tgid: PId,
    child_exit: WaitQueue,
    container: Arc<Container>,
    vmspace: VmSpace,
    mutable: SpinLock<Mutable>,
    fd_table: SpinLock<FdTable>,
    signal_wait: WaitQueue,
}

impl Process {
    pub fn new_init(
        container: Arc<Container>,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<Arc<Process>, Errno> {
        let vmspace = container.root_vmspace.try_clone()?;
        let (mappings, brk, entry, sp) = create_address_space(&vmspace, elf_file, argv)?;

        let mut fd_table = FdTable::new(1024); // TODO: make this configurable
        let console: Arc<dyn FileLike> = Arc::new(Console::new());
        fd_table.insert_at(0, console.clone(), O_RDONLY)?;
        fd_table.insert_at(1, console.clone(), O_WRONLY)?;
        fd_table.insert_at(2, console, O_WRONLY)?;

        let process = Self::new(
            container,
            vmspace,
            fd_table,
            PId(1),
            None,
            mappings,
            brk,
            entry,
            sp,
            SignalMap::new(SigAction::default()),
            |_thread| Ok(()),
        )?;

        Ok(process)
    }

    pub fn exec(
        self: &Arc<Self>,
        current: &LxThread,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<(), Errno> {
        // Unmap the old mappings.
        // TODO: Can we build a new VM space from scratch?
        let old_mappings = core::mem::take(&mut self.mutable.lock().mappings);
        for mapping in &old_mappings {
            self.vmspace.unmap(mapping.start, mapping.len)?;
        }

        let (mappings, brk, entry, sp) = create_address_space(&self.vmspace, elf_file, argv)?;

        self.fd_table.lock().close_on_exec();

        let thread = LxThread::new(
            &self.container.hspace,
            &self.vmspace,
            entry,
            sp,
            Arc::downgrade(self),
            self.tgid,
        )?;
        thread.start()?;

        let mut mutable = self.mutable.lock();
        mutable.mappings = mappings;
        mutable.brk = brk;
        mutable
            .threads
            .retain(|thread| !core::ptr::eq(thread.as_ref(), current));
        mutable.threads.push(thread);
        Ok(())
    }

    fn new<F>(
        container: Arc<Container>,
        vmspace: VmSpace,
        fd_table: FdTable,
        tgid: PId,
        parent: Option<Weak<Process>>,
        mappings: Vec<Mapping>,
        brk: Brk,
        entry: usize,
        sp: usize,
        signal_actions: SignalMap<SigAction>,
        thread_prestart: F,
    ) -> Result<Arc<Self>, Errno>
    where
        F: FnOnce(&Arc<LxThread>) -> Result<(), Errno>,
    {
        let child_exit = WaitQueue::new()?;
        let signal_wait = WaitQueue::new()?;
        let process = Arc::new(Self {
            tgid,
            child_exit,
            container: container.clone(),
            vmspace,
            mutable: SpinLock::new(Mutable {
                parent,
                threads: Vec::with_capacity(1),
                mappings,
                brk,
                children: Vec::new(),
                exit_status: None,
                signal_actions,
                pending_signals: SignalSet::empty(),
            }),
            fd_table: SpinLock::new(fd_table),
            signal_wait,
        });

        // TODO: LX assumes that the cookie won't be dereferenced until the
        // thread is started. Should we document and guarantee this?
        let thread = LxThread::new(
            &container.hspace,
            &process.vmspace,
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

    pub fn fork(
        self: &Arc<Self>,
        current: &LxThread,
        frame: &mut SyscallFrame,
    ) -> Result<PId, Errno> {
        let vmspace = self.container.root_vmspace.try_clone()?;
        let fd_table = self.fd_table.lock().clone();

        // Set the return value for the child process.
        frame.set_retval(0);
        let syscall_sp = frame as *const SyscallFrame as usize;

        // Copy memory into the child's VM space.
        // TODO: copy on write
        let mutable = self.mutable.lock();
        let mappings = mutable.mappings.clone();
        let brk = mutable.brk;
        let signal_actions = mutable.signal_actions.fork();
        drop(mutable);
        for mapping in &mappings {
            let vmo = Vmo::create(mapping.len)?;
            let bytes =
                unsafe { core::slice::from_raw_parts(mapping.start as *const u8, mapping.len) };
            vmo.write(0, bytes)?;
            vmspace.map(&vmo, mapping.start, mapping.attrs)?;
        }

        // Allocate a new PID for the child process.
        let mut pid_table = self.container.processes.lock();
        let tgid = pid_table.allocate()?;

        // Create a new process and the first thread.
        let entry = restore_regs as *const () as usize;
        let child = Self::new(
            self.container.clone(),
            vmspace,
            fd_table,
            tgid,
            Some(Arc::downgrade(self)),
            mappings,
            brk,
            entry,
            syscall_sp,
            signal_actions,
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

        // Wake up the parent process while holding the lock. When notification
        // fails, exit fails and keeps this process alive.
        let parent = mutable.parent.as_ref().and_then(Weak::upgrade);
        if let Some(parent) = &parent {
            parent.child_exit.notify_all()?;
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

        drop(mutable);

        // Close all file descriptors.
        self.fd_table.lock().clear();
        Ok(())
    }

    pub fn wait(&self, pid: c_int) -> Result<(PId, c_int), Errno> {
        if pid != -1 && pid <= 0 {
            return Err(Errno::EINVAL);
        }

        let mut wq = WaitSet::new()?;
        wq.subscribe(&self.child_exit);
        wq.subscribe(&self.signal_wait);
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

            if !mutable.pending_signals.is_empty() {
                return Err(Errno::EINTR);
            }

            drop(mutable);
            wq.wait()?;
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
        if addr < brk.start || addr > BRK_END {
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

        mutable.brk.current_aligned = current_aligned;
        addr
    }

    pub fn mmap(
        &self,
        _addr: usize,
        len: usize,
        prot: c_int,
        flags: c_int,
        _fd: c_int,
        _offset: i64,
    ) -> Result<usize, Errno> {
        if flags & MAP_ANONYMOUS == 0 {
            // TODO: MAP_FIXED is not supported yet.
            return Err(Errno::ENOSYS);
        }

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
        self.vmspace.map(&vmo, uaddr, attrs)?;

        // Record the mapping.
        self.mutable.lock().mappings.push(Mapping {
            start: uaddr,
            len,
            attrs,
        });

        Ok(uaddr)
    }

    pub fn fd_table(&self) -> &SpinLock<FdTable> {
        &self.fd_table
    }

    pub fn id(&self) -> PId {
        self.tgid
    }

    pub fn sigaction(
        &self,
        signal: Signal,
        new_action: Option<SigAction>,
    ) -> Result<SigAction, Errno> {
        let mut mutable = self.mutable.lock();

        let old = *mutable.signal_actions.get(signal);
        if let Some(action) = new_action {
            mutable.signal_actions.set(signal, action);
        }

        Ok(old)
    }

    pub fn queue_signal(&self, signal: Signal) -> Result<(), Errno> {
        let mut mutable = self.mutable.lock();
        let action = *mutable.signal_actions.get(signal);
        match action.disposition() {
            SigDisposition::Ignore => return Ok(()),
            SigDisposition::Default => {
                trace!("unsupproted default handling for signal {:?}", signal);
                return Err(Errno::ENOTSUP);
            }
            SigDisposition::Handler(_) => {}
        }

        mutable.pending_signals.insert(signal);
        drop(mutable);
        self.signal_wait.notify_all()?;
        Ok(())
    }

    pub fn has_pending_signal(&self) -> bool {
        !self.mutable.lock().pending_signals.is_empty()
    }

    pub fn take_pending_signal(&self) -> Option<(Signal, SigAction)> {
        let mut mutable = self.mutable.lock();
        let signal = mutable.pending_signals.take_first()?;
        let action = *mutable.signal_actions.get(signal);
        Some((signal, action))
    }

    pub fn signal_wait_queue(&self) -> &WaitQueue {
        &self.signal_wait
    }

    pub fn container(&self) -> &Arc<Container> {
        &self.container
    }
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

fn create_address_space(
    vmspace: &VmSpace,
    elf_file: Arc<dyn FileLike>,
    argv: &[&[u8]],
) -> Result<(Vec<Mapping>, Brk, usize, usize), Errno> {
    let mut mappings = Vec::new();
    let elf = load_elf(vmspace, elf_file.as_ref(), &mut mappings)?;

    // Find the end of the program segments.
    // TODO: Can we guarantee that mappings is not empty?
    let brk_start = mappings
        .iter()
        .map(|mapping| mapping.start + mapping.len)
        .max()
        .unwrap_or(0);

    let stack = Vmo::create(STACK_SIZE)?;
    let sp = prepare_stack(&stack, STACK_BOTTOM, STACK_SIZE, argv, &elf)?;

    vmspace.map(&stack, STACK_BOTTOM, PageAttrs::READ | PageAttrs::WRITE)?;
    mappings.push(Mapping {
        start: STACK_BOTTOM,
        len: STACK_SIZE,
        attrs: PageAttrs::READ | PageAttrs::WRITE,
    });

    Ok((mappings, Brk::new(brk_start), elf.entry, sp))
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
