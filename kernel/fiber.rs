use alloc::boxed::Box;
use alloc::sync::Arc;
use core::fmt;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use super::scheduler::GLOBAL_SCHEDULER;
use crate::arch;
use crate::channel::Channel;
use crate::lock::Mutex;
use crate::scheduler::Scheduler;

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
    pub fn new() -> Self {
        Self {
            id: FiberId::alloc(),
            state: FiberState::Blocked,
            ctx: arch::Context::zeroed(),
            handles: HashMap::new(),
        }
    }

    // FIXME: what if the handle already exists?
    pub fn insert_handle(&mut self, handle: HandleId, object: Object) {
        self.handles.insert(handle, object);
    }

    pub fn spawn_in_kernel<F>(self, f: F) -> Arc<Mutex<Fiber>>
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        self.do_spawn(Box::new(f))
    }

    fn do_spawn(mut self, f: Box<dyn FnOnce()>) -> Arc<Mutex<Fiber>> {
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

        self.ctx = arch::Context::new_kernel(pc, arg);
        let fiber = Arc::new(Mutex::new(self));

        GLOBAL_SCHEDULER.lock().resume(fiber.clone());
        fiber
    }

    pub fn new_idle() -> Self {
        Self {
            id: FiberId::alloc(),
            state: FiberState::Blocked,
            ctx: arch::Context::zeroed(),
            handles: HashMap::new(),
        }
    }

    pub fn get_channel_by_handle(handle: HandleId) -> Result<Channel, FtlError> {
        let current = arch::cpuvar_ref().current.lock();
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

pub struct WaitQueue {
    fiber: Option<Arc<Mutex<Fiber>>>,
}

impl WaitQueue {
    pub fn new() -> Self {
        Self { fiber: None }
    }

    pub fn wake(self) {
        if let Some(fiber) = self.fiber {
            GLOBAL_SCHEDULER.lock().resume(fiber);
        }
    }

    pub fn sleep<'a, T>(&mut self) {
        debug_assert!(self.fiber.is_none());

        let current = arch::cpuvar_ref().current.clone();
        GLOBAL_SCHEDULER.lock().block(&current);
        self.fiber = Some(current);

        // TODO: drop(guard);
        arch::yield_cpu();
    }
}
