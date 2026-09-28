use alloc::sync::Arc;

use ftl::hspace::HandleSpace;
use ftl::poll::Poll;
use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::vmspace::VmSpace;
use ftl_types::poll::EventKind;
use ftl_types::time::Duration;
use ftl_utils::spinlock::SpinLock;
use pid_table::PIdTable;

use crate::net::TcpIp;
use crate::process::PId;
use crate::process::Process;
use crate::reaper::Reaper;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::FileLike;

mod pid_table;

pub struct Container {
    poll: Poll,
    pub hspace: HandleSpace,
    pub root_vmspace: VmSpace,
    pub processes: SpinLock<PIdTable>,
    pub reaper: Reaper,
    network: Arc<TcpIp>,
    console: Arc<Console>,
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
        let poll = Poll::create()?;
        network.subscribe(&poll)?;
        console.subscribe(&poll)?;

        let this = Arc::new(Self {
            poll,
            hspace,
            root_vmspace,
            processes: SpinLock::new(PIdTable::new()),
            reaper: Reaper::new()?,
            network,
            console: console.clone(),
        });

        let init_process = Process::new_init(this.clone(), console, elf_file, argv)?;
        this.processes.lock().insert(PId::new(1), init_process);
        Ok(this)
    }

    pub fn network(&self) -> &Arc<TcpIp> {
        &self.network
    }

    pub fn add_thread(&self, thread: Arc<LxThread>) -> Result<(), Errno> {
        self.reaper.start_thread(&self.poll, thread.clone())?;
        Ok(())
    }

    pub fn run(&self) -> ! {
        loop {
            let deadline = MonoTime::now() + Duration::from_secs(1);
            let event = self.poll.wait_until(deadline).expect("poll wait failed");
            if event.kind() == EventKind::ThreadExited {
                self.reaper.reap_thread(event.handle_id());
            } else if event.handle_id() == self.network.id() {
                self.network.handle_rx();
                self.network
                    .subscribe(&self.poll)
                    .expect("failed to subscribe to network events");
            } else if event.handle_id() == self.console.id() {
                self.console.handle_rx();
                self.console
                    .subscribe(&self.poll)
                    .expect("failed to subscribe to console events");
            }

            self.network.handle_timeouts(MonoTime::now());
        }
    }
}
