use core::fmt;
use core::time::Duration;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_TIME_NOW;
use ftl_types::syscall::SYS_TIMER_CREATE;
use ftl_types::syscall::SYS_TIMER_SET;
use ftl_types::time::Monotonic;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall2;

#[derive(Clone, Copy)]
pub struct Instant(Monotonic);

impl Instant {
    pub fn now() -> Self {
        Self(sys_time_now().expect("failed to get time"))
    }

    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).map(Self)
    }

    pub fn is_before(&self, other: &Instant) -> bool {
        self.0.is_before(&other.0)
    }

    pub fn elapsed_since(&self, other: &Instant) -> Duration {
        self.0.elapsed_since(&other.0)
    }
}

pub struct Timer {
    handle: OwnedHandle,
}

impl Timer {
    pub fn new() -> Result<Timer, ErrorCode> {
        let handle = sys_timer_create()?;
        Ok(Timer { handle })
    }

    pub fn set_timeout(&self, at: Instant) -> Result<(), ErrorCode> {
        sys_timer_set(self.handle.id(), at)
    }
}

impl Handleable for Timer {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn into_handle(self) -> OwnedHandle {
        self.handle
    }
}

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Timer")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

pub fn sys_time_now() -> Result<Monotonic, ErrorCode> {
    let raw = syscall0(SYS_TIME_NOW)?;
    Ok(Monotonic::from_nanos(raw as u64))
}

pub fn sys_timer_create() -> Result<OwnedHandle, ErrorCode> {
    let id = syscall0(SYS_TIMER_CREATE)?;
    Ok(OwnedHandle::from_raw(HandleId::from_raw(id)))
}

pub fn sys_timer_set(handle: HandleId, at: Instant) -> Result<(), ErrorCode> {
    syscall2(
        SYS_TIMER_SET,
        handle.as_usize(),
        at.0.as_raw() as usize, // TODO: Check overflow
    )?;
    Ok(())
}
