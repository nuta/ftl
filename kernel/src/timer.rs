use alloc::vec::Vec;
use core::mem;
use core::time::Duration;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventBody;
use ftl_types::sink::EventType;
use ftl_types::sink::TimerEvent;
use ftl_types::time::Monotonic;
use ftl_utils::static_assert;

use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::EventEmitter;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

#[derive(Debug, Clone)]
enum State {
    NotSet,
    Expired,
    Pending(Monotonic),
}

struct Mutable {
    state: State,
    emitter: Option<EventEmitter>,
}

pub struct Timer {
    mutable: SpinLock<Mutable>,
}

impl Timer {
    pub fn new() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::NotSet,
                emitter: None,
            }),
        })
    }

    pub fn set_timeout(self: &SharedRef<Self>, duration: Duration) -> Result<(), ErrorCode> {
        let mut global_timer = GLOBAL_TIMER.lock();
        let now = arch::read_timer();
        let expires_at = now
            .checked_add(duration)
            .ok_or(ErrorCode::InvalidArgument)?;

        let mut mutable = self.mutable.lock();
        let old_state = mem::replace(&mut mutable.state, State::Pending(expires_at));
        drop(mutable);

        if matches!(old_state, State::NotSet) {
            global_timer.actives.push(self.clone());
        }

        reschedule_timer(&global_timer);
        Ok(())
    }
}

impl Handleable for Timer {
    fn set_event_emitter(&self, emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable.emitter = emitter;
        Ok(())
    }

    fn read_event(
        &self,
        _handle_table: &mut HandleTable,
    ) -> Result<Option<(EventType, EventBody)>, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::Expired) {
            return Ok(None);
        }

        mutable.state = State::NotSet;
        let body = EventBody {
            timer: TimerEvent {},
        };
        Ok(Some((EventType::TIMER, body)))
    }

    fn close(&self) {
        self.mutable.lock().emitter = None;
    }
}

struct GlobalTimer {
    actives: Vec<SharedRef<Timer>>,
}

impl GlobalTimer {
    pub const fn new() -> Self {
        Self {
            actives: Vec::new(),
        }
    }
}

static GLOBAL_TIMER: SpinLock<GlobalTimer> = SpinLock::new(GlobalTimer::new());

// Reschedule for the next earliest timer.
fn reschedule_timer(global_timer: &GlobalTimer) {
    // Find the earliest timer.
    let mut earliest = None;
    for timer in &global_timer.actives {
        let mutable = timer.mutable.lock();
        if let State::Pending(expires_at) = mutable.state {
            if matches!(earliest, None)
                || matches!(
                    earliest,
                    Some(earliest_at) if expires_at.is_before(&earliest_at)
                )
            {
                earliest = Some(expires_at);
            }
        }
    }

    if let Some(deadline) = earliest {
        arch::set_timer(deadline);
    }
}

pub fn handle_interrupt() {
    let now = arch::read_timer();
    let mut global_timer = GLOBAL_TIMER.lock();

    // Check all timers and remove expired ones.
    let mut new_actives = Vec::new();
    for timer in &global_timer.actives {
        let mut mutable = timer.mutable.lock();
        match mutable.state {
            State::Pending(expires_at) if expires_at.is_before(&now) => {
                // The timer has expired, notify the listeners.
                mutable.state = State::Expired;
                if let Some(emitter) = mutable.emitter.as_mut() {
                    emitter.notify();
                }
            }
            State::Pending(_) => {
                // The timer is still pending, keep it in the active list.
                new_actives.push(timer.clone());
            }
            _ => {
                unreachable!("timer is active but not pending");
            }
        }
    }

    global_timer.actives = new_actives;
    reschedule_timer(&global_timer);
}

pub fn sys_time_now() -> Result<SyscallResult, ErrorCode> {
    let now = arch::read_timer();
    // FIXME: Use a user slice to return the time.
    static_assert!(size_of::<u64>() == size_of::<usize>());
    Ok(SyscallResult::Return(now.as_nanos() as usize))
}

pub fn sys_timer_create(thread: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    let timer = Timer::new()?;
    let handle = Handle::new(timer, HandleRight::ALL);
    let id = thread.process().handle_table().lock().insert(handle)?;
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_timer_set(
    thread: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let timer_id = HandleId::from_raw(a0);
    let timeout_nanos = a1 as u64;
    static_assert!(size_of::<u64>() == size_of::<usize>());

    thread
        .process()
        .handle_table()
        .lock()
        .get::<Timer>(timer_id)?
        .authorize(HandleRight::WRITE)?
        .set_timeout(Duration::from_nanos(timeout_nanos))?;

    Ok(SyscallResult::Return(0))
}
