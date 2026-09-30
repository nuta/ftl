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
use crate::types::errno::Errno;
use crate::types::sys::futex::FUTEX_BITSET_MATCH_ANY;
use crate::vm::Vm;

struct Mutable {
    signal_frame: Option<SyscallFrame>,
    exit_status: Option<c_int>,
    clear_child_tid: Option<usize>,
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
                    clear_child_tid: None,
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

    /// Cleans up an exited thread.
    ///
    /// Note: Call this in LX's main loop.
    pub fn reap(&self) -> Result<(), Errno> {
        let Some(process) = self.process.upgrade() else {
            return Ok(());
        };

        // Clear the TID, and wake up the waiter.
        let clear_child_tid = self.mutable.lock().clear_child_tid;
        if let Some(uaddr) = clear_child_tid {
            if let Err(err) = self.clear_child_tid(&process, uaddr) {
                trace!("failed to clear child TID at {:#x}: {:?}", uaddr, err);
            }
        }

        process.on_thread_exit(self)
    }

    /// Note: Call this in LX's main loop.
    fn clear_child_tid(&self, process: &Process, uaddr: usize) -> Result<(), Errno> {
        // Don't access uaddr directly. It is in a different address space
        // since we're in the LX's main loop.
        self.vm.write(uaddr, &0u32.to_ne_bytes())?;
        process.futexes().wake(uaddr, 1, FUTEX_BITSET_MATCH_ANY)?;
        Ok(())
    }

    pub fn set_clear_child_tid(&self, uaddr: Option<usize>) {
        self.mutable.lock().clear_child_tid = uaddr;
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
