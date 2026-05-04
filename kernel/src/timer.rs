use alloc::vec::Vec;
use core::mem;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;
use ftl_types::time::Monotonic;
use ftl_utils::static_assert;

use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::Notifier;
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
    notifier: Option<Notifier>,
}

pub struct Timer {
    mutable: SpinLock<Mutable>,
}

impl Timer {
    pub fn new() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                state: State::NotSet,
                notifier: None,
            }),
        })
    }

    pub fn set_timeout(self: &SharedRef<Self>, expires_at: Monotonic) -> Result<(), ErrorCode> {
        let now = arch::read_timer();

        let mut mutable = self.mutable.lock();
        if !now.is_before(&expires_at) {
            // The timer has already expired, make it readable immediately.
            let old_state = mem::replace(&mut mutable.state, State::Expired);
            if let Some(ref notifier) = mutable.notifier {
                notifier.notify();
            }
            drop(mutable);

            if matches!(old_state, State::Pending(_)) {
                GLOBAL_TIMER.lock().remove(self);
            }

            return Ok(());
        }

        let old_state = mem::replace(&mut mutable.state, State::Pending(expires_at));
        drop(mutable);

        let mut global_timer = GLOBAL_TIMER.lock();
        if !matches!(old_state, State::Pending(_)) {
            global_timer.add(self.clone());
        } else {
            global_timer.reschedule();
        }

        Ok(())
    }
}

impl Handleable for Timer {
    fn set_notifier(&self, notifier: Notifier) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.notifier.is_some() {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable.notifier = Some(notifier);
        Ok(())
    }

    fn remove_notifier(&self) {
        let mut mutable = self.mutable.lock();
        debug_assert!(mutable.notifier.is_some());
        mutable.notifier = None;
    }

    fn poll(
        &self,
        handle_id: HandleId,
        _handle_table: &mut HandleTable,
        isolation: &SharedRef<dyn Isolation>,
        buf: &UserSlice,
    ) -> Result<bool, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::Expired) {
            return Ok(false);
        }

        mutable.state = State::NotSet;

        crate::isolation::write(
            isolation,
            buf,
            0,
            EventHeader {
                ty: EventType::TIMER,
                id: handle_id,
                reserved: 0,
            },
        )?;

        Ok(true)
    }

    fn close(&self) {
        self.mutable.lock().notifier = None;
    }
}

static GLOBAL_TIMER: SpinLock<GlobalTimer> = SpinLock::new(GlobalTimer::new());

struct GlobalTimer {
    actives: Vec<SharedRef<Timer>>,
}

impl GlobalTimer {
    pub const fn new() -> Self {
        Self {
            actives: Vec::new(),
        }
    }

    fn add(&mut self, timer: SharedRef<Timer>) {
        self.actives.push(timer);
        self.reschedule();
    }

    fn remove(&mut self, timer: &SharedRef<Timer>) {
        self.actives.retain(|active| !SharedRef::eq(active, timer));
        self.reschedule();
    }

    // Reschedule for the next earliest timer.
    fn reschedule(&mut self) {
        // Find the earliest timer.
        let mut earliest = None;
        for timer in &self.actives {
            let mutable = timer.mutable.lock();
            if let State::Pending(expires_at) = mutable.state
                && (earliest.is_none()
                    || matches!(
                        earliest,
                        Some(earliest_at) if expires_at.is_before(&earliest_at)
                    ))
            {
                earliest = Some(expires_at);
            }
        }

        if let Some(deadline) = earliest {
            arch::set_timer(deadline);
        }
    }

    fn notify_expired_timers(&mut self, now: Monotonic) {
        // Check all timers and remove expired ones.
        let mut new_actives = Vec::new();
        for timer in &self.actives {
            let mut mutable = timer.mutable.lock();
            match mutable.state {
                State::Pending(expires_at) if !now.is_before(&expires_at) => {
                    // The timer has expired, notify the listeners.
                    mutable.state = State::Expired;
                    if let Some(ref notifier) = mutable.notifier {
                        notifier.notify();
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

        self.actives = new_actives;
    }
}

pub fn handle_interrupt() {
    let now = arch::read_timer();
    let mut global_timer = GLOBAL_TIMER.lock();
    global_timer.notify_expired_timers(now);
}

pub fn sys_time_now() -> Result<SyscallResult, ErrorCode> {
    let now = arch::read_timer();
    // FIXME: Use a user slice to return the time.
    static_assert!(size_of::<u64>() == size_of::<usize>());
    Ok(SyscallResult::Return(now.as_raw() as usize))
}

pub fn sys_timer_create(thread: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    let mut handle_table = thread.process().handle_table().lock();
    let slot = handle_table.reserve()?;

    let timer = Timer::new()?;
    let handle = Handle::new(timer, HandleRight::ALL);
    let id = slot.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_timer_set(
    thread: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let timer_id = HandleId::from_raw(a0);
    let expires_at = Monotonic::from_nanos(a1 as u64);
    static_assert!(size_of::<u64>() == size_of::<usize>());

    thread
        .process()
        .handle_table()
        .lock()
        .get::<Timer>(timer_id)?
        .authorize(HandleRight::WRITE)?
        .set_timeout(expires_at)?;

    Ok(SyscallResult::Return(0))
}
