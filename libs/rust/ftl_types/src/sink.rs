use crate::channel::CallId;
use crate::channel::MessageBody;
use crate::channel::MessageInfo;
use crate::handle::HandleId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventType(u32);

impl EventType {
    pub const MESSAGE: Self = Self(1);
    pub const IRQ: Self = Self(2);
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawEvent {
    pub header: EventHeader,
    pub body: EventBody,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IrqEvent {
    pub irq: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union EventBody {
    pub message: MessageEvent,
    pub irq: IrqEvent,
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
    pub call_id: CallId,
    pub body: MessageBody,
}
