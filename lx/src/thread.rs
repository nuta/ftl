use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;

use ftl::isolate::Isolate;
use ftl::thread::Thread;
use ftl::vmspace::VmSpace;
use ftl_types::error::ErrorCode;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;

use crate::process::PId;
use crate::process::Process;

pub struct LxThread {
    process: Weak<Process>,
    tid: PId,
    inner: Thread,
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
        isolate: &Isolate,
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
        let inner = Thread::create(isolate, vmspace, entry, sp, fault_pc, cookie)?;

        let thread = Arc::new(LxThread {
            process,
            tid,
            inner,
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

    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in
    /// [`LxThread::new`].
    pub unsafe fn from_cookie(cookie: usize) -> Arc<LxThread> {
        unsafe { Cookie::from_raw(cookie) }
    }
}
