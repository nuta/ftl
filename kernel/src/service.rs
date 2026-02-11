use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;

use ftl_arrayvec::ArrayString;
use ftl_types::error::ErrorCode;
use ftl_types::sink::ClientEvent;
use ftl_types::sink::EventBody;
use ftl_types::sink::EventType;

use crate::channel::Channel;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::EventEmitter;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Promise;
use crate::thread::Thread;

pub const SERVICE_NAME_MAX_LEN: usize = 32;

struct Mutable {
    emitter: Option<EventEmitter>,
    new_connections: VecDeque<SharedRef<Channel>>,
}

pub struct Service {
    mutable: SpinLock<Mutable>,
}

impl Service {
    pub fn register(name: ArrayString<SERVICE_NAME_MAX_LEN>) -> Result<SharedRef<Self>, ErrorCode> {
        let service = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                emitter: None,
                new_connections: VecDeque::new(),
            }),
        })?;

        let mut services = SERVICES.lock();
        let mut wait_queues = WAIT_QUEUES.lock();
        if let Some(wq) = wait_queues.remove(&name) {
            for thread in wq.threads {
                thread.unblock();
            }
        };

        services.insert(name, service.clone());
        Ok(service)
    }

    pub fn lookup(
        current: &SharedRef<Thread>,
        name: &ArrayString<SERVICE_NAME_MAX_LEN>,
    ) -> Result<Option<SharedRef<Channel>>, ErrorCode> {
        let services = SERVICES.lock();
        match services.get(&name) {
            Some(service) => {
                let mut mutable = service.mutable.lock();
                let (client_ch, server_ch) = Channel::new()?;
                mutable.new_connections.push_back(server_ch);

                // Tell the server process that a new client has connected.
                if let Some(ref emitter) = mutable.emitter {
                    emitter.notify();
                }
                Ok(Some(client_ch))
            }
            None => {
                WAIT_QUEUES
                    .lock()
                    .entry(name.clone())
                    .or_insert(WaitQueue::new())
                    .threads
                    .push_back(current.clone());
                Ok(None)
            }
        }
    }
}

impl Handleable for Service {
    fn set_event_emitter(&self, emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable.emitter = emitter;
        Ok(())
    }

    fn read_event(
        &self,
        handle_table: &mut HandleTable,
    ) -> Result<Option<(EventType, EventBody)>, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(ch) = mutable.new_connections.pop_front() else {
            return Ok(None);
        };

        let id = handle_table.insert(Handle::new(ch, HandleRight::ALL))?;
        let body = EventBody {
            client: ClientEvent { id },
        };

        Ok(Some((EventType::CLIENT, body)))
    }
}

// Lock order: SERVICES -> WAIT_QUEUES
static SERVICES: SpinLock<BTreeMap<ArrayString<SERVICE_NAME_MAX_LEN>, SharedRef<Service>>> =
    SpinLock::new(BTreeMap::new());
static WAIT_QUEUES: SpinLock<BTreeMap<ArrayString<SERVICE_NAME_MAX_LEN>, WaitQueue>> =
    SpinLock::new(BTreeMap::new());

struct WaitQueue {
    threads: VecDeque<SharedRef<Thread>>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self {
            threads: VecDeque::new(),
        }
    }
}

fn read_name_from_user(
    isolation: &SharedRef<dyn Isolation>,
    slice: &UserSlice,
) -> Result<ArrayString<SERVICE_NAME_MAX_LEN>, ErrorCode> {
    if slice.len() > SERVICE_NAME_MAX_LEN {
        return Err(ErrorCode::InvalidArgument);
    }

    let mut name_buf = [0u8; SERVICE_NAME_MAX_LEN];
    let name_bytes = &mut name_buf[..slice.len()];
    isolation.read_bytes(slice, name_bytes)?;
    ArrayString::from_ascii_str(name_bytes).map_err(|_| ErrorCode::InvalidArgument)
}

pub fn sys_service_register(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let name_slice = UserSlice::new(UserPtr::new(a0), a1)?;

    let process = current.process();
    let name = read_name_from_user(process.isolation(), &name_slice)?;

    let service = Service::register(name)?;
    let mut handle_table = process.handle_table().lock();
    let id = handle_table.insert(Handle::new(service, HandleRight::ALL))?;

    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_service_lookup(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let name_slice = UserSlice::new(UserPtr::new(a0), a1)?;

    let process = current.process();
    let name = read_name_from_user(process.isolation(), &name_slice)?;
    let Some(ch) = Service::lookup(current, &name)? else {
        return Ok(SyscallResult::Blocked(Promise::ServiceLookup { name }));
    };

    let mut handle_table = process.handle_table().lock();
    let id = handle_table.insert(Handle::new(ch, HandleRight::ALL))?;

    Ok(SyscallResult::Return(id.as_usize()))
}
