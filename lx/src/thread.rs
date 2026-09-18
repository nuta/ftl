use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;

use ftl::hspace::HandleSpace;
use ftl::thread::Thread;
use ftl::trace;
use ftl::vmspace::VmSpace;
use ftl_types::error::ErrorCode;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_utils::spinlock::SpinLock;

use crate::arch::SyscallFrame;
use crate::process::PId;
use crate::process::Process;
use crate::signal::SigDisposition;
use crate::types::c_long;
use crate::types::errno::Errno;

struct Mutable {
    signal_frame: Option<SyscallFrame>,
}

pub struct LxThread {
    process: Weak<Process>,
    tid: PId,
    inner: Thread,
    mutable: SpinLock<Mutable>,
}

struct Cookie {
    thread: Arc<LxThread>,
}

impl Cookie {
    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in
    /// [`LxThread::new`].
    unsafe fn from_raw(cookie: usize) -> Arc<LxThread> {
        let ptr = cookie as *const Cookie;
        unsafe { (*ptr).thread.clone() }
    }
}

impl LxThread {
    pub fn new(
        hspace: &HandleSpace,
        vmspace: &VmSpace,
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
        let inner = Thread::create(hspace, vmspace, entry, sp, fault_pc, cookie)?;

        let thread = Arc::new(LxThread {
            process,
            tid,
            inner,
            mutable: SpinLock::new(Mutable { signal_frame: None }),
        });

        // Initialize and leak the thread context. We'll free manually later.
        Box::leak(Box::write(
            this,
            Cookie {
                thread: thread.clone(),
            },
        ));

        Ok(thread)
    }

    pub fn start(&self) -> Result<(), ErrorCode> {
        self.inner.start()
    }

    pub fn tid(&self) -> PId {
        self.tid
    }

    pub fn process(&self) -> Arc<Process> {
        self.process.upgrade().unwrap()
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
    /// [`LxThread::new`].
    pub unsafe fn from_cookie(cookie: usize) -> Arc<LxThread> {
        unsafe { Cookie::from_raw(cookie) }
    }
}
