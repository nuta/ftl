use crate::channel::MessageBody;
use crate::channel::MessageInfo;
use crate::handle::HandleId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventType(u32);

impl EventType {
    pub const MESSAGE: Self = Self(1);
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union Event {
    pub message: MessageEvent,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct EventHeader {
    pub ty: EventType,
    pub id: HandleId,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MessageEvent {
    pub info: MessageInfo,
    pub cookie: usize,
    pub body: MessageBody,
}
