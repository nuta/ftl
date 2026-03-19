use core::fmt;

use crate::channel::MessageInfo;
use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EventType(u32);

impl EventType {
    pub const MESSAGE: Self = Self(1);
    pub const IRQ: Self = Self(2);
    pub const PEER_CLOSED: Self = Self(3);
    pub const TIMER: Self = Self(4);
    pub const SANDBOXED_SYSCALL: Self = Self(5);
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union Event {
    pub common: EventHeader,
    pub message: MessageEvent,
    pub irq: IrqEvent,
    pub peer_closed: PeerClosedEvent,
    pub timer: TimerEvent,
    pub sandboxed_syscall: SandboxedSyscallEvent,
}

impl Event {
    pub fn header(&self) -> EventHeader {
        unsafe { self.common }
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.header().ty {
            EventType::MESSAGE => f.debug_struct("MessageEvent").finish(),
            EventType::IRQ => f.debug_struct("IrqEvent").finish(),
            EventType::PEER_CLOSED => f.debug_struct("PeerClosedEvent").finish(),
            EventType::TIMER => f.debug_struct("TimerEvent").finish(),
            EventType::SANDBOXED_SYSCALL => f.debug_struct("SandboxedSyscallEvent").finish(),
            _ => f.debug_struct("UnknownEvent").finish(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EventHeader {
    pub ty: EventType,
    pub id: HandleId,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MessageEvent {
    pub header: EventHeader,
    pub info: MessageInfo,
    pub arg1: usize,
    pub arg2: usize,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TimerEvent {
    pub header: EventHeader,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct IrqEvent {
    pub header: EventHeader,
    pub irq: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PeerClosedEvent {
    pub header: EventHeader,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SandboxedSyscallEvent {
    pub header: EventHeader,
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
