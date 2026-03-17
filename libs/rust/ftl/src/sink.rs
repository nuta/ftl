use core::fmt;
use core::mem::MaybeUninit;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
pub use ftl_types::sink::Event;
pub use ftl_types::sink::SandboxedSyscallEvent;
use ftl_types::syscall::SYS_SINK_ADD;
use ftl_types::syscall::SYS_SINK_CREATE;
use ftl_types::syscall::SYS_SINK_REMOVE;
use ftl_types::syscall::SYS_SINK_WAIT;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall2;

pub struct Sink {
    handle: OwnedHandle,
}

impl Sink {
    pub fn new() -> Result<Sink, ErrorCode> {
        let handle = sys_sink_create()?;
        Ok(Sink { handle })
    }

    pub fn add<H: Handleable>(&self, handle: &H) -> Result<(), ErrorCode> {
        sys_sink_add(self.handle.id(), handle.handle().id())?;
        Ok(())
    }

    pub fn remove(&self, id: HandleId) -> Result<(), ErrorCode> {
        sys_sink_remove(self.handle.id(), id)?;
        Ok(())
    }

    pub fn wait<'a>(&self, buf: &'a mut MaybeUninit<Event>) -> Result<&'a Event, ErrorCode> {
        sys_sink_wait(self.handle.id(), buf)?;
        // SAFETY: The buffer is initialized by the kernel.
        Ok(unsafe { buf.assume_init_ref() })
    }
}

impl Handleable for Sink {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn into_handle(self) -> OwnedHandle {
        self.handle
    }
}

impl fmt::Debug for Sink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sink")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

fn sys_sink_create() -> Result<OwnedHandle, ErrorCode> {
    let handle = syscall0(SYS_SINK_CREATE)?;
    Ok(OwnedHandle::from_raw(HandleId::from_raw(handle)))
}

fn sys_sink_add(sink: HandleId, handle: HandleId) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_ADD, sink.as_usize(), handle.as_usize())?;
    Ok(())
}

fn sys_sink_remove(sink: HandleId, id: HandleId) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_REMOVE, sink.as_usize(), id.as_usize())?;
    Ok(())
}

fn sys_sink_wait(sink: HandleId, buf: &mut MaybeUninit<Event>) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_WAIT, sink.as_usize(), buf.as_mut_ptr() as usize)?;
    Ok(())
}
