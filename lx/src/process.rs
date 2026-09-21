use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::fmt;

use ftl::trace;
use ftl_types::thread::RegsKind;
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
use crate::types::sys::fcntl::O_CLOEXEC;
use crate::types::sys::fcntl::O_RDONLY;
use crate::types::sys::fcntl::O_WRONLY;
use crate::vfs::Console;
use crate::vfs::FileLike;
use crate::vfs::Tty;
use crate::vm::Vm;
use crate::wait_queue::WaitQueue;
use crate::wait_queue::WaitSet;

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

    /// Inserts two files.
    pub fn insert2(
        &mut self,
        file1: Arc<dyn FileLike>,
        flags1: c_int,
        file2: Arc<dyn FileLike>,
        flags2: c_int,
    ) -> Result<(c_int, c_int), Errno> {
        let fd1 = self.insert(file1, flags1)?;
        let fd2 = match self.insert(file2, flags2) {
            Ok(fd) => fd,
            Err(error) => {
                let _ = self.remove(fd1);
                return Err(error);
            }
        };

        Ok((fd1, fd2))
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

    fn find_free_fd(&self, minfd: c_int) -> Result<usize, Errno> {
        if minfd < 0 {
            return Err(Errno::EINVAL);
        }

        for fd in (minfd as usize)..self.capacity {
            if fd >= self.open_files.len() || self.open_files[fd].is_none() {
                return Ok(fd);
            }
        }

        Err(Errno::EMFILE)
    }

    /// Duplicates `oldfd` to `newfd`.
    fn do_dup(&mut self, oldfd: c_int, newfd: usize, cloexec: bool) -> Result<c_int, Errno> {
        let file = self.get(oldfd)?.clone();
        if newfd >= self.open_files.len() {
            self.open_files.resize(newfd + 1, None);
        }

        let old = self.open_files[newfd].replace(Entry { file, cloexec });
        if old.is_none() {
            self.active_fds += 1;
        }

        Ok(newfd as c_int)
    }

    pub fn dup(&mut self, oldfd: c_int, minfd: c_int, cloexec: bool) -> Result<c_int, Errno> {
        let newfd = self.find_free_fd(minfd)?;
        self.do_dup(oldfd, newfd, cloexec)
    }

    pub fn dup2(&mut self, oldfd: c_int, newfd: c_int) -> Result<c_int, Errno> {
        // > If oldfd is a valid file descriptor, and newfd has the same
        // > value as oldfd, then dup2() does nothing, and returns newfd.
        // >
        // > https://man7.org/linux/man-pages/man2/dup.2.html
        if oldfd == newfd {
            // Check the validity of oldfd.
            self.get(oldfd)?;
            return Ok(newfd);
        }

        self.dup3(oldfd, newfd, 0)
    }

    pub fn dup3(&mut self, oldfd: c_int, newfd: c_int, flags: c_int) -> Result<c_int, Errno> {
        if flags & !O_CLOEXEC != 0 {
            // Reject unsupported flags.
            return Err(Errno::EINVAL);
        }

        if oldfd == newfd {
            // Unlike dup2:
            //
            // > If oldfd equals newfd, then dup3() fails with the error EINVAL.
            // >
            // > https://man7.org/linux/man-pages/man2/dup.2.html
            return Err(Errno::EINVAL);
        }

        if newfd < 0 || (newfd as usize) >= self.capacity {
            return Err(Errno::EBADF);
        }

        self.do_dup(oldfd, newfd as usize, flags & O_CLOEXEC != 0)
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
    children: Vec<Arc<Process>>,
    exit_status: Option<c_int>,
    signal_actions: SignalMap<SigAction>,
    pending_signals: SignalSet,
}

pub struct Process {
    tgid: PId,
    child_exit: WaitQueue,
    container: Arc<Container>,
    mutable: SpinLock<Mutable>,
    fd_table: SpinLock<FdTable>,
    signal_wait: WaitQueue,
}

impl Process {
    pub fn new_init(
        container: Arc<Container>,
        console: Arc<Console>,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<Arc<Process>, Errno> {
        let vmspace = container.root_vmspace.try_clone()?;
        let (vm, entry, sp) = Vm::create(&vmspace, elf_file, argv)?;

        let mut fd_table = FdTable::new(1024); // TODO: make this configurable
        let tty: Arc<dyn FileLike> = Arc::new(Tty::new(console));
        fd_table.insert_at(0, tty.clone(), O_RDONLY)?;
        fd_table.insert_at(1, tty.clone(), O_WRONLY)?;
        fd_table.insert_at(2, tty, O_WRONLY)?;

        let process = Self::new(
            container,
            vm,
            fd_table,
            PId(1),
            None,
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
        let (vm, entry, sp) = Vm::create(&self.container.root_vmspace, elf_file, argv)?;
        self.fd_table.lock().close_on_exec();

        let thread = LxThread::new(
            &self.container.hspace,
            Arc::new(vm),
            entry,
            sp,
            Arc::downgrade(self),
            self.tgid,
        )?;
        thread.start()?;

        let mut mutable = self.mutable.lock();
        mutable
            .threads
            .retain(|thread| !core::ptr::eq(thread.as_ref(), current));
        mutable.threads.push(thread);
        Ok(())
    }

    fn new<F>(
        container: Arc<Container>,
        vm: Vm,
        fd_table: FdTable,
        tgid: PId,
        parent: Option<Weak<Process>>,
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
            mutable: SpinLock::new(Mutable {
                parent,
                threads: Vec::with_capacity(1),
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
            Arc::new(vm),
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
        let fd_table = self.fd_table.lock().clone();

        // Set the return value for the child process.
        frame.set_retval(0);
        let syscall_sp = frame as *const SyscallFrame as usize;

        // Copy memory into the child's VM space.
        // TODO: copy on write
        let mutable = self.mutable.lock();
        let new_vm = current.vm().fork(&self.container.root_vmspace)?;
        let signal_actions = mutable.signal_actions.fork();
        drop(mutable);

        // Allocate a new PID for the child process.
        let mut pid_table = self.container.processes.lock();
        let tgid = pid_table.allocate()?;

        // Create a new process and the first thread.
        let entry = restore_regs as *const () as usize;
        let child = Self::new(
            self.container.clone(),
            new_vm,
            fd_table,
            tgid,
            Some(Arc::downgrade(self)),
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

    pub fn wait(&self, pid: c_int, wnohang: bool) -> Result<Option<(PId, c_int)>, Errno> {
        if pid != -1 && pid <= 0 {
            return Err(Errno::EINVAL);
        }

        let mut wq = None;
        if !wnohang {
            let mut set = WaitSet::new()?;
            set.subscribe(&self.child_exit);
            set.subscribe(&self.signal_wait);
            wq = Some(set);
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
                    return Ok(Some((tgid, status)));
                }

                matched_any = true;
            }

            if !matched_any {
                return Err(Errno::ECHILD);
            }

            let Some(wq) = &wq else {
                // WNOHANG is set. Return immediately.
                return Ok(None);
            };

            if !mutable.pending_signals.is_empty() {
                return Err(Errno::EINTR);
            }

            drop(mutable);
            wq.wait()?;
        }
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
