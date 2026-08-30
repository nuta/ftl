use alloc::sync::Arc;

use ftl_types::handle::HandleId;
use ftl_utils::spinlock::SpinLock;
use pid_table::PIdTable;

use crate::net::TcpIp;
use crate::process::PId;
use crate::process::Process;
use crate::types::errno::Errno;
use crate::vfs::FileLike;

mod pid_table;

pub struct Container {
    pub isolate: HandleId,
    pub root_vmspace: HandleId,
    pub processes: SpinLock<PIdTable>,
    network: SpinLock<Option<Arc<TcpIp>>>,
}

impl Container {
    pub fn new(
        isolate: HandleId,
        root_vmspace: HandleId,
        elf_file: Arc<dyn FileLike>,
    ) -> Result<Arc<Self>, Errno> {
        let this = Arc::new(Self {
            isolate,
            root_vmspace,
            processes: SpinLock::new(PIdTable::new()),
            network: SpinLock::new(None),
        });

        let init_process = Process::new_init(this.clone(), elf_file)?;
        this.processes.lock().insert(PId::new(1), init_process);
        Ok(this)
    }

    pub fn set_network(&self, network: Arc<TcpIp>) {
        *self.network.lock() = Some(network);
    }

    pub fn network(&self) -> Arc<TcpIp> {
        self.network
            .lock()
            .as_ref()
            .expect("network service is not initialized")
            .clone()
    }
}
