use crate::handle::HandleId;
use crate::vmspace::UserCopyable;

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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EventHeader {
    pub ty: EventType,
    pub reserved: u32,
    pub id: HandleId,
}

// SAFETY: The `EventHeader` does not have padding.
unsafe impl UserCopyable for EventHeader {}

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

// SAFETY: The `SyscallRegs` does not have padding.
unsafe impl UserCopyable for SyscallRegs {}
