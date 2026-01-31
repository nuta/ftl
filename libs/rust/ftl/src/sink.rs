use core::mem::MaybeUninit;

use ftl_types::channel::CallId;
use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::Event;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;
use ftl_types::sink::MessageEvent;
use ftl_types::syscall::SYS_SINK_ADD;
use ftl_types::syscall::SYS_SINK_CREATE;
use ftl_types::syscall::SYS_SINK_WAIT;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall2;

/// A received message from a channel via sink.
pub struct ReceivedMessage {
    pub handle_id: HandleId,
    pub info: MessageInfo,
    pub call_id: CallId,
    pub cookie: usize,
    pub inline: [u8; ftl_types::channel::INLINE_LEN_MAX],
}

pub struct Sink {
    handle: OwnedHandle,
}

impl Sink {
    pub fn new() -> Result<Self, ErrorCode> {
        let id = sys_sink_create()?;
        Ok(Self {
            handle: OwnedHandle::from_raw(id),
        })
    }

    pub fn add<T: Handleable>(&self, object: &T) -> Result<(), ErrorCode> {
        sys_sink_add(self.handle.id(), object.handle().id())
    }

    pub fn wait(&self) -> Result<ReceivedMessage, ErrorCode> {
        let mut event: MaybeUninit<(EventHeader, Event)> = MaybeUninit::uninit();
        sys_sink_wait(self.handle.id(), event.as_mut_ptr() as usize)?;

        let (header, event) = unsafe { event.assume_init() };
        assert!(header.ty == EventType::MESSAGE, "unexpected event type");

        let message: MessageEvent = unsafe { event.message };
        Ok(ReceivedMessage {
            handle_id: header.id,
            info: message.info,
            call_id: message.call_id,
            cookie: message.cookie,
            inline: message.body.inline,
        })
    }
}

impl Handleable for Sink {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

fn sys_sink_create() -> Result<HandleId, ErrorCode> {
    let id = syscall0(SYS_SINK_CREATE)?;
    Ok(HandleId::from_raw(id))
}

fn sys_sink_add(sink_id: HandleId, object_id: HandleId) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_ADD, sink_id.as_usize(), object_id.as_usize())?;
    Ok(())
}

fn sys_sink_wait(sink_id: HandleId, buf: usize) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_WAIT, sink_id.as_usize(), buf)?;
    Ok(())
}

