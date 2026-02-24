use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::prelude::vec::Vec;
use ftl::process::Process;
use ftl::sync::Arc;
use ftl::thread::Thread;
use ftl::vmarea::VmArea;
use ftl::vmspace::VmSpace;

use crate::thread::LxThread;

pub enum CreateError {
    VmSpaceCreate(ErrorCode),
    ProcessCreate(ErrorCode),
    ThreadSpawn(crate::thread::Error),
}

pub enum UserCopyError {
    NotMappedAddr,
    ReadVmArea(ErrorCode),
}

struct Vma {
    vmarea: VmArea,
    base: usize,
    end: usize,
}

impl Vma {
    pub fn new(vmarea: VmArea, base: usize, end: usize) -> Self {
        Self { vmarea, base, end }
    }
}

struct Mutable {
    threads: VecDeque<Arc<LxThread>>,
    vmas: Vec<Vma>,
}

pub struct LxProcess {
    mutable: spin::Mutex<Mutable>,
    /// `None` for the root process.
    parent: Option<Arc<LxProcess>>,
    process: Process,
    vmspace: VmSpace,
}

impl LxProcess {
    pub fn create(parent: Option<Arc<LxProcess>>) -> Result<Arc<Self>, CreateError> {
        let vmspace = VmSpace::new().map_err(CreateError::VmSpaceCreate)?;
        let process = Process::create_sandboxed(&vmspace, "linux_compat")
            .map_err(CreateError::ProcessCreate)?;

        let entry = todo!("read entry from ELF");

        let process = Arc::new(Self {
            mutable: spin::Mutex::new(Mutable {
                threads: VecDeque::with_capacity(1),
                vmas: Vec::new(),
            }),
            parent,
            process,
            vmspace,
        });
        LxThread::spawn(process.clone(), entry).map_err(CreateError::ThreadSpawn)?;

        Ok(process)
    }

    pub(crate) fn ftl_process(&self) -> &Process {
        &self.process
    }

    pub fn add_thread(&self, thread: Arc<LxThread>) {
        self.mutable.lock().threads.push_back(thread);
    }

    pub fn read_from_user(&self, addr: usize, buf: &mut [u8]) -> Result<(), UserCopyError> {
        let mutable = self.mutable.lock();
        let mut pos = 0;
        while pos < buf.len() {
            let cur = addr + pos;
            let vma = mutable
                .vmas
                .iter()
                .find(|vma| vma.base <= cur && cur < vma.end)
                .ok_or(UserCopyError::NotMappedAddr)?;
            let span = core::cmp::min(buf.len() - pos, vma.end - cur);
            vma.vmarea
                .read(cur - vma.base, &mut buf[pos..pos + span])
                .map_err(UserCopyError::ReadVmArea)?;
            pos += span;
        }

        Ok(())
    }
}
