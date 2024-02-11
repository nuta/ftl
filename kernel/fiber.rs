use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use ftl_types::{error::FtlError, handle::HandleId};
use hashbrown::HashMap;

use crate::{
    arch::{self, cpuvar_ref},
    channel::Channel,
    lock::Mutex,
    scheduler::Scheduler,
};

use super::scheduler::GLOBAL_SCHEDULER;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);

impl FiberId {
    pub fn alloc() -> FiberId {
        // TODO: wrap around and check for duplicates
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Display for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

pub enum Object {
    Channel(Channel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiberState {
    Runnable,
    Blocked,
}

pub struct Fiber {
    id: FiberId,
    state: FiberState,
    ctx: arch::Context,
    handles: HashMap<HandleId, Object>,
}

impl Fiber {
    pub fn spawn<F>(f: F) -> Arc<Mutex<Fiber>>
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Fiber::do_spawn(Box::new(f))
    }

    fn do_spawn(f: Box<dyn FnOnce()>) -> Arc<Mutex<Fiber>> {
        let id = FiberId::alloc();

        extern "C" fn native_entry(arg: *mut Box<dyn FnOnce()>) {
            let closure = unsafe { Box::from_raw(arg) };
            closure();
            Scheduler::exit_current(GLOBAL_SCHEDULER.lock());
        }

        let main = move || {
            f();
            println!("fiber {} exited", id);
        };

        let pc = native_entry as usize;
        let closure = Box::into_raw(Box::new(main));
        let arg = closure as usize;
        let fiber = Fiber::new_kernel(id, pc, arg);
        let fiber = Arc::new(Mutex::new(fiber));

        GLOBAL_SCHEDULER.lock().resume(fiber.clone());
        fiber
    }

    pub fn new_idle() -> Self {
        Self {
            id: FiberId::alloc(),
            state: FiberState::Blocked,
            ctx: arch::Context::new_idle(),
            handles: HashMap::new(),
        }
    }

    pub fn new_kernel(id: FiberId, pc: usize, arg: usize) -> Self {
        Self {
            id,
            state: FiberState::Blocked,
            ctx: arch::Context::new_kernel(pc, arg),
            handles: HashMap::new(),
        }
    }

    pub fn get_channel_by_handle(handle: HandleId) -> Result<Channel, FtlError> {
        let current = cpuvar_ref().current.lock();
        let object = current
            .handles
            .get(&handle)
            .ok_or(FtlError::InvalidHandle)?;

        match object {
            Object::Channel(channel) => Ok(channel.clone()),
        }
    }

    pub fn id(&self) -> FiberId {
        self.id
    }

    pub fn state(&self) -> FiberState {
        self.state
    }

    pub unsafe fn context_mut_ptr(&mut self) -> *mut arch::Context {
        &mut self.ctx as *mut arch::Context
    }

    pub fn set_state(&mut self, new_state: FiberState) {
        debug_assert!(self.state != new_state);
        self.state = new_state;
    }
}
