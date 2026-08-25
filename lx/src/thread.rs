use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;

use ftl::syscall::thread_create;
use ftl::syscall::thread_start;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::process::PId;
use crate::process::Process;

#[derive(Debug)]
pub enum SpawnError {
    ThreadCreate(ErrorCode),
}

pub struct Thread {
    process: Weak<Process>,
    tid: PId,
    handle: HandleId,
}

pub struct ThreadCtx {
    pub thread: Arc<Thread>,
}

impl ThreadCtx {
    /// # Safety
    ///
    /// `cookie` must be the thread's cookie which we created in [`Thread::new`].
    pub unsafe fn from_cookie(cookie: usize) -> Arc<Thread> {
        let ptr = cookie as *const ThreadCtx;
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
    ) -> Result<Arc<Self>, SpawnError> {
        let this = Box::<ThreadCtx>::new_uninit();
        let fault_pc = crate::arch::syscall_handler as *const () as usize;
        let cookie = this.as_ptr() as usize;

        // TODO: LX assumes that the cookie won't be derefernced until the
        //       thread is started. Should we document and guarantee this?
        let handle = thread_create(isolate, vmspace, entry, sp, fault_pc, cookie)
            .map_err(SpawnError::ThreadCreate)?;

        let thread = Arc::new(Thread {
            process,
            tid,
            handle,
        });

        // Initialize and leak the thread context. We'll free manually later.
        Box::leak(Box::write(
            this,
            ThreadCtx {
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
}
