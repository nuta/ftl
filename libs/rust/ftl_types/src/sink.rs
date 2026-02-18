use crate::channel::CallId;
use crate::channel::MessageBody;
use crate::channel::MessageInfo;
use crate::handle::HandleId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventType(u32);

impl EventType {
    pub const MESSAGE: Self = Self(1);
    pub const IRQ: Self = Self(2);
    pub const PEER_CLOSED: Self = Self(3);
    pub const TIMER: Self = Self(4);
    pub const CLIENT: Self = Self(5);
    pub const SYSCALL: Self = Self(6);
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
pub struct PeerClosedEvent {}

#[derive(Clone, Copy)]
#[repr(C)]
pub union EventBody {
    pub message: MessageEvent,
    pub irq: IrqEvent,
    pub peer_closed: PeerClosedEvent,
    pub timer: TimerEvent,
    pub client: ClientEvent,
    pub syscall: SyscallEvent,
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

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TimerEvent {}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ClientEvent {
    /// The channel ID connected to the client.
    pub id: HandleId,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SyscallEvent {
    pub thread_id: HandleId,
    pub regs: SyscallRegs,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SyscallRegs {
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8: u64,
    pub r9: u64,
}
