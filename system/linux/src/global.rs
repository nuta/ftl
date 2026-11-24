use alloc::sync::Arc;

use ftl::mutex::Mutex;
use ftl::poll::Poll;
use ftl::quota::Quota;
use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HANDLE_ID_MAX;
use ftl_types::handle::HandleId;
use ftl_types::time::Duration;

use crate::net::TcpIp;
use crate::pid_table::PIdTable;
use crate::process::PId;
use crate::process::Process;
use crate::reaper::Reaper;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::vfs::Console;
use crate::vfs::FileLike;
use crate::vfs::PathNode;
use crate::vfs::RootDir;

// `cookie <= HANDLE_ID_MAX` are for threads.
const NETWORK_COOKIE: usize = HANDLE_ID_MAX + 1;
const CONSOLE_COOKIE: usize = HANDLE_ID_MAX + 2;

pub struct Global {
    poll: Poll,
    pub quota: Arc<Quota>,
    pub root_vmspace: VmSpace,
    pub processes: Mutex<PIdTable>,
    pub reaper: Reaper,
    network: Arc<TcpIp>,
    console: Option<Arc<Console>>,
    pub root_dir: Arc<PathNode>,
    init_exit_status: Mutex<Option<c_int>>,
}

impl Global {
    pub fn new(
        quota: Quota,
        root_vmspace: VmSpace,
        image: Vmo,
        network: Arc<TcpIp>,
        console: Option<Arc<Console>>,
        stdio: Option<(Arc<dyn FileLike>, Arc<dyn FileLike>)>,
        elf_file: Arc<dyn FileLike>,
        argv: &[&[u8]],
    ) -> Result<Arc<Self>, Errno> {
        let poll = Poll::create()?;

        network.subscribe(&poll, NETWORK_COOKIE)?;

        if let Some(console) = &console {
            console.subscribe(&poll, CONSOLE_COOKIE)?;
        }

        let quota = Arc::new(quota);
        let root_dir = RootDir::new(quota.clone(), image, network.clone());
        let this = Arc::new(Self {
            poll,
            quota,
            root_vmspace,
            processes: Mutex::new(PIdTable::new()),
            reaper: Reaper::new()?,
            network,
            console: console.clone(),
            root_dir: PathNode::root_dir(root_dir),
            init_exit_status: Mutex::new(None),
        });

        let init_process = Process::new_init(this.clone(), console, stdio, elf_file, argv)?;
        this.processes.lock().insert(PId::new(1), init_process);
        Ok(this)
    }

    pub fn network(&self) -> &Arc<TcpIp> {
        &self.network
    }

    pub fn add_thread(&self, thread: Arc<Thread>, entry: usize, sp: usize) -> Result<(), Errno> {
        self.reaper
            .start_thread(&self.poll, thread.clone(), entry, sp)?;
        Ok(())
    }

    pub fn set_init_exit_status(&self, status: c_int) {
        self.init_exit_status.lock().get_or_insert(status);
    }

    pub fn init_exit_status(&self) -> Option<c_int> {
        *self.init_exit_status.lock()
    }

    pub fn handle_events(&self) {
        let now = MonoTime::now();
        let deadline = match self.network.handle_timeouts(now) {
            Some(ack_deadline) => ack_deadline,
            None => now + Duration::from_secs(1),
        };

        match self.poll.wait_until(deadline) {
            Ok(NETWORK_COOKIE) => {
                self.network.handle_rx();
                self.network
                    .subscribe(&self.poll, NETWORK_COOKIE)
                    .expect("failed to subscribe to network events");
            }
            Ok(CONSOLE_COOKIE) if let Some(console) = &self.console => {
                console.handle_rx();
                console
                    .subscribe(&self.poll, CONSOLE_COOKIE)
                    .expect("failed to subscribe to console events");
            }
            Ok(thread_id) if thread_id <= HANDLE_ID_MAX => {
                self.reaper.reap_thread(HandleId::new(thread_id))
            }
            Ok(cookie) => panic!("unexpected poll cookie: {:#x}", cookie),
            Err(ErrorCode::TimedOut) => {}
            Err(err) => panic!("poll wait failed: {:?}", err),
        }
    }
}
