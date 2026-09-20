use alloc::sync::Arc;

use ftl::hspace::HandleSpace;
use ftl::vmspace::VmSpace;
use ftl_utils::spinlock::SpinLock;
use pid_table::PIdTable;

use crate::net::TcpIp;
use crate::process::PId;
use crate::process::Process;
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::FileLike;

mod pid_table;

pub struct Container {
    pub hspace: HandleSpace,
    pub root_vmspace: VmSpace,
    pub processes: SpinLock<PIdTable>,
    network: Arc<TcpIp>,
}

impl Container {
    pub fn new(
        hspace: HandleSpace,
        root_vmspace: VmSpace,
        network: Arc<TcpIp>,
        console: Arc<Console>,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<Arc<Self>, Errno> {
        let this = Arc::new(Self {
            hspace,
            root_vmspace,
            processes: SpinLock::new(PIdTable::new()),
            network,
        });

        let init_process = Process::new_init(this.clone(), console, elf_file, argv)?;
        this.processes.lock().insert(PId::new(1), init_process);
        Ok(this)
    }

    pub fn network(&self) -> &Arc<TcpIp> {
        &self.network
    }
}
