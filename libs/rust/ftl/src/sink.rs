use alloc::boxed::Box;
use core::fmt;
use core::mem::MaybeUninit;

use ftl_arrayvec::ArrayVec;
use ftl_types::channel::CallId;
use ftl_types::channel::INLINE_LEN_MAX;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::NUM_HANDLES_MAX;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventType;
use ftl_types::sink::RawEvent;
use ftl_types::syscall::SYS_SINK_ADD;
use ftl_types::syscall::SYS_SINK_CREATE;
use ftl_types::syscall::SYS_SINK_WAIT;

use crate::channel::Cookie;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall0;
use crate::syscall::syscall2;

pub enum Event {
    CallMessage {
        ch_id: HandleId,
        info: MessageInfo,
        call_id: CallId,
        handles: ArrayVec<OwnedHandle, NUM_HANDLES_MAX>,
        inline: [u8; INLINE_LEN_MAX],
    },
    ReplyMessage {
        ch_id: HandleId,
        info: MessageInfo,
        cookie: Box<Cookie>,
        handles: ArrayVec<OwnedHandle, NUM_HANDLES_MAX>,
        inline: [u8; INLINE_LEN_MAX],
    },
    Irq {
        handle_id: HandleId,
        irq: u8,
    },
}

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

    pub fn wait(&self) -> Result<Event, ErrorCode> {
        let mut buf = MaybeUninit::<RawEvent>::uninit();
        sys_sink_wait(self.handle.id(), &mut buf)?;

        // SAFETY: The buffer is initialized by the kernel.
        let raw = unsafe { buf.assume_init() };
        let event = match raw.header.ty {
            EventType::MESSAGE => {
                // SAFETY: Checked that the event type is MESSAGE.
                let message = unsafe { &raw.body.message };
                let info = message.info;
                let mut handles = ArrayVec::new();
                for i in 0..info.num_handles() {
                    let id = message.body.handles[i];
                    handles.try_push(OwnedHandle::from_raw(id)).unwrap();
                }

                // TODO: Do not copy the entire inline array.
                if info.is_call() {
                    Event::CallMessage {
                        ch_id: raw.header.id,
                        info,
                        call_id: message.call_id,
                        handles,
                        inline: message.body.inline,
                    }
                } else {
                    // FIXME: Cookie is not guaranteed to be Box<Cookie>.
                    let cookie = unsafe { Cookie::from_raw(message.cookie) };
                    Event::ReplyMessage {
                        ch_id: raw.header.id,
                        info,
                        cookie,
                        handles,
                        inline: message.body.inline,
                    }
                }
            }
            EventType::IRQ => {
                let irq_event = unsafe { &raw.body.irq };
                Event::Irq {
                    handle_id: raw.header.id,
                    irq: irq_event.irq,
                }
            }
            _ => {
                return Err(ErrorCode::Unsupported);
            }
        };

        Ok(event)
    }
}

impl Handleable for Sink {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
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

fn sys_sink_wait(sink: HandleId, buf: &mut MaybeUninit<RawEvent>) -> Result<(), ErrorCode> {
    syscall2(SYS_SINK_WAIT, sink.as_usize(), buf.as_mut_ptr() as usize)?;
    Ok(())
}
