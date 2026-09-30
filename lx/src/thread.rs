use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;

use ftl::hspace::HandleSpace;
use ftl::poll::Poll;
use ftl::thread::Thread;
use ftl::trace;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_utils::spinlock::SpinLock;

use crate::arch::SyscallFrame;
use crate::process::PId;
use crate::process::Process;
use crate::signal::SigDisposition;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::signal::SIGCHLD;
use crate::types::sys::sched::CLONE_FLAGS_MASK;
use crate::vm::Vm;

struct Mutable {
    signal_frame: Option<SyscallFrame>,
    exit_status: Option<c_int>,
}

pub struct LxThread {
    process: Weak<Process>,
    vm: Arc<Vm>,
    tid: PId,
    inner: Thread,
    mutable: SpinLock<Mutable>,
    _cookie: Box<Cookie>,
}

struct Cookie {
    thread: Weak<LxThread>,
}

impl LxThread {
    pub fn new(
        hspace: &HandleSpace,
        vm: Arc<Vm>,
        entry: usize,
        sp: usize,
        process: Weak<Process>,
        tid: PId,
    ) -> Result<Arc<Self>, ErrorCode> {
        let this = Box::<Cookie>::new_uninit();
        let fault_pc = crate::arch::syscall_handler as *const () as usize;
        let cookie = this.as_ptr() as usize;

        // TODO: LX assumes that the cookie won't be derefernced until the
        //       thread is started. Should we document and guarantee this?
        let inner = Thread::create(hspace, vm.vmspace(), entry, sp, fault_pc, cookie)?;

        let thread = Arc::new_cyclic(|thread| {
            LxThread {
                process,
                vm,
                tid,
                inner,
                mutable: SpinLock::new(Mutable {
                    signal_frame: None,
                    exit_status: None,
                }),
                _cookie: Box::write(
                    this,
                    Cookie {
                        thread: thread.clone(),
                    },
                ),
            }
        });

        Ok(thread)
    }

    pub fn start(&self) -> Result<(), ErrorCode> {
        self.inner.start()
    }

    pub fn id(&self) -> HandleId {
        self.inner.id()
    }

    pub fn subscribe(&self, poll: &Poll) -> Result<(), ErrorCode> {
        self.inner.subscribe(poll)
    }

    pub fn tid(&self) -> PId {
        self.tid
    }

    pub fn process(&self) -> Arc<Process> {
        self.process.upgrade().unwrap()
    }

    pub fn on_exit(&self) -> Result<(), Errno> {
        if let Some(process) = self.process.upgrade() {
            process.on_thread_exit(self)?;
        }

        Ok(())
    }

    pub fn exit_status(&self) -> Option<c_int> {
        self.mutable.lock().exit_status
    }

    pub fn set_exit_status(&self, status: c_int) {
        self.mutable.lock().exit_status = Some(status);
    }

    pub fn vm(&self) -> &Arc<Vm> {
        &self.vm
    }

    pub fn set_fsbase(&self, fsbase: usize) -> Result<(), ErrorCode> {
        self.inner
            .write_regs(RegsKind::FsBase, Regs { fs_base: fsbase })
    }

    pub fn copy_regs_to(&self, dest: &LxThread, kind: RegsKind) -> Result<(), ErrorCode> {
        self.inner.copy_regs_to(&dest.inner, kind)
    }

    pub fn do_clone(&self, frame: &mut SyscallFrame, flags: c_ulong) -> Result<PId, Errno> {
        const SUPPORTED_FLAGS: c_ulong = 0;

        let exit_signal = (flags & CLONE_FLAGS_MASK) as c_int;
        if exit_signal != SIGCHLD {
            trace!("clone: unsupported exit signal {}", exit_signal);
            return Err(Errno::EINVAL);
        }

        let flags = flags & !CLONE_FLAGS_MASK;
        if flags & !SUPPORTED_FLAGS != 0 {
            trace!("clone: unsupported flags {:#x}", flags & !SUPPORTED_FLAGS);
            return Err(Errno::EINVAL);
        }

        self.process().fork(self, frame)
    }

    /// Modifies the thread's state to return to the signal handler.
    pub fn handle_pending_signal(&self, frame: &mut SyscallFrame) {
        let mut mutable = self.mutable.lock();
        if mutable.signal_frame.is_some() {
            // A signal is already being delivered.
            return;
        }

        let process = self.process();
        let (signal, action, handler) = loop {
            let Some((signal, action)) = process.take_pending_signal() else {
                // No pending signal to deliver.
                return;
            };

            match action.disposition() {
                SigDisposition::Handler(handler) => break (signal, action, handler),
                SigDisposition::Ignore => {
                    // Handle the next pending signal.
                    continue;
                }
                SigDisposition::Default => {
                    trace!(
                        "default signal handling for {:?} is not implemented",
                        signal
                    );

                    // Handle the next pending signal.
                    continue;
                }
            }
        };

        // Save the original system call frame. We'll restore it when returning
        // from the signal handler.
        mutable.signal_frame = Some(*frame);

        unsafe {
            frame.enter_signal(signal.number() as usize, handler, action.restorer());
        }
    }

    /// Restores the thread's state to resume from signal handling.
    pub fn return_from_signal(&self, frame: &mut SyscallFrame) -> Result<c_long, Errno> {
        let mut mutable = self.mutable.lock();
        let saved = mutable.signal_frame.take().ok_or(Errno::EINVAL)?;
        *frame = saved;
        Ok(saved.retval())
    }

    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in
    /// [`LxThread::new`]. Also, the caller must ensure that the thread is
    /// not freed.
    pub unsafe fn from_cookie<'a>(cookie: usize) -> &'a LxThread {
        let cookie = unsafe { &*(cookie as *const Cookie) };
        unsafe { &*cookie.thread.as_ptr() }
    }
}
