#[repr(usize)]
pub enum Syscall {
    HandleClose = 0,
    ConsoleOpen = 1,
    ConsoleRead = 2,
    ConsoleWrite = 3,
    ConsoleSubscribe = 4,
    ThreadCreate = 5,
    ThreadStart = 6,
    ThreadWriteRegs = 7,
    ThreadCopyRegs = 8,
    ThreadSubscribe = 9,
    ThreadExit = 10,
    VmSpaceClone = 11,
    VmSpaceMap = 12,
    VmSpaceUnmap = 13,
    VmSpacePermit = 14,
    VmoCreate = 15,
    VmoRead = 16,
    VmoWrite = 17,
    PollCreate = 18,
    PollWait = 19,
    PollWaitUntil = 20,
    PollNotify = 21,
    NetCreate = 22,
    NetBind = 23,
    NetUnbind = 24,
    NetRecv = 25,
    NetSend = 26,
    NetSubscribe = 27,
    MonoTimeRead = 28,
    WallTimeRead = 29,
    RandomRead = 30,
    VmoCreateUser = 31,
    VmoSupply = 32,
    VmoSnapshot = 33,
    // Note: Do not forget to update Syscall::END when adding a new syscall.
}

impl Syscall {
    pub const BASE: usize = usize::MAX - 0x1000;
    const END: usize = Self::BASE + Self::VmoSnapshot as usize;

    pub const fn as_usize(self) -> usize {
        Self::BASE + self as usize
    }

    pub fn from_usize(value: usize) -> Option<Self> {
        if (Self::BASE..=Self::END).contains(&value) {
            // SAFETY: Discriminants are in [BASE..=END].
            Some(unsafe { core::mem::transmute::<usize, Self>(value - Self::BASE) })
        } else {
            None
        }
    }
}
