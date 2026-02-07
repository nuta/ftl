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

pub struct Instant(Monotonic);

impl Instant {
    pub fn now() -> Self {
        Self(sys_time_now().expect("failed to get time"))
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
}

impl Handleable for Timer {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
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

pub fn sys_timer_set(handle: HandleId, duration: Duration) -> Result<(), ErrorCode> {
    syscall2(
        SYS_TIMER_SET,
        handle.as_usize(),
        duration.as_millis() as usize,
    )?;
    Ok(())
}
