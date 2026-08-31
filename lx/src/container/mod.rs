use alloc::sync::Arc;

use ftl::isolate::Isolate;
use ftl::vmspace::VmSpace;
use ftl_utils::spinlock::SpinLock;
use pid_table::PIdTable;

use crate::net::TcpIp;
use crate::process::PId;
use crate::process::Process;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

mod pid_table;

pub struct Container {
    pub isolate: Isolate,
    pub root_vmspace: VmSpace,
    pub processes: SpinLock<PIdTable>,
    network: Arc<TcpIp>,
}

impl Container {
    pub fn new(
        isolate: Isolate,
        root_vmspace: VmSpace,
        network: Arc<TcpIp>,
        elf_file: Arc<dyn FileLike>,
    ) -> Result<Arc<Self>, Errno> {
        let this = Arc::new(Self {
            isolate,
            root_vmspace,
            processes: SpinLock::new(PIdTable::new()),
            network,
        });

        let init_process = Process::new_init(this.clone(), elf_file)?;
        this.processes.lock().insert(PId::new(1), init_process);
        Ok(this)
    }

    pub fn network(&self) -> &Arc<TcpIp> {
        &self.network
    }
}
