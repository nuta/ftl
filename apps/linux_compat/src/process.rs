use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::process::Process;
use ftl::sync::Arc;
use ftl::thread::Thread;
use ftl::vmspace::VmSpace;

use crate::thread::LxThread;

pub enum Error {
    VmSpaceCreate(ErrorCode),
    ProcessCreate(ErrorCode),
    ThreadSpawn(crate::thread::Error),
}

struct Mutable {
    threads: VecDeque<Arc<LxThread>>,
}

pub struct LxProcess {
    mutable: spin::Mutex<Mutable>,
    /// `None` for the root process.
    parent: Option<Arc<LxProcess>>,
    process: Process,
    vmspace: VmSpace,
}

impl LxProcess {
    pub fn create(parent: Option<Arc<LxProcess>>) -> Result<Arc<Self>, Error> {
        let vmspace = VmSpace::new().map_err(Error::VmSpaceCreate)?;
        let process =
            Process::create_sandboxed(&vmspace, "linux_compat").map_err(Error::ProcessCreate)?;

        let entry = todo!("read entry from ELF");

        let process = Arc::new(Self {
            mutable: spin::Mutex::new(Mutable {
                threads: VecDeque::with_capacity(1),
            }),
            parent,
            process,
            vmspace,
        });
        LxThread::spawn(process.clone(), entry).map_err(Error::ThreadSpawn)?;

        Ok(process)
    }

    pub fn ftl_process(&self) -> &Process {
        &self.process
    }

    pub fn add_thread(&self, thread: Arc<LxThread>) {
        self.mutable.lock().threads.push_back(thread);
    }
}
