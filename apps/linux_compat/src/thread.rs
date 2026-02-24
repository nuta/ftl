use ftl::error::ErrorCode;
use ftl::sync::Arc;
use ftl::thread::Thread;

use crate::process::LxProcess;

pub enum Error {
    ThreadCreate(ErrorCode),
    ThreadResume(ErrorCode),
}

pub struct LxThread {
    process: Arc<LxProcess>,
    thread: Thread,
}

impl LxThread {
    pub fn spawn(process: Arc<LxProcess>, entry: usize) -> Result<(), Error> {
        let sp = todo!("allocate stack");
        let thread =
            Thread::create(process.ftl_process(), entry, sp, 0).map_err(Error::ThreadCreate)?;
        let this = Arc::new(Self { process, thread });
        process.add_thread(this.clone());
        Ok(())
    }

    pub fn resume_with(&self, retval: usize) -> Result<(), ErrorCode> {
        self.thread.resume_with(retval)
    }
}
