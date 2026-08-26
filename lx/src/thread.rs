use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;

use ftl::syscall::thread_copy_regs;
use ftl::syscall::thread_create;
use ftl::syscall::thread_start;
use ftl::syscall::thread_write_regs;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;

use crate::process::PId;
use crate::process::Process;

pub struct Thread {
    process: Weak<Process>,
    tid: PId,
    handle: HandleId,
}

struct Cookie {
    thread: Arc<Thread>,
}

impl Cookie {
    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in
    /// [`Thread::new`].
    unsafe fn from_raw(cookie: usize) -> Arc<Thread> {
        let ptr = cookie as *const Cookie;
        unsafe { (*ptr).thread.clone() }
    }
}

impl Thread {
    pub fn new(
        isolate: HandleId,
        vmspace: HandleId,
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
        let handle = thread_create(isolate, vmspace, entry, sp, fault_pc, cookie)?;

        let thread = Arc::new(Thread {
            process,
            tid,
            handle,
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
        thread_start(self.handle)
    }

    pub fn tid(&self) -> PId {
        self.tid
    }

    pub fn process(&self) -> Arc<Process> {
        self.process.upgrade().unwrap()
    }

    pub fn set_fsbase(&self, fsbase: usize) -> Result<(), ErrorCode> {
        thread_write_regs(self.handle, RegsKind::FsBase, Regs { fs_base: fsbase })
    }

    pub fn copy_regs_to(&self, dest: &Thread, kind: RegsKind) -> Result<(), ErrorCode> {
        thread_copy_regs(self.handle, dest.handle, kind)
    }

    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in
    /// [`Thread::new`].
    pub unsafe fn from_cookie(cookie: usize) -> Arc<Thread> {
        unsafe { Cookie::from_raw(cookie) }
    }
}
