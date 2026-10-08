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
    VmoCreateZeroed = 15,
    VmoCreateUser = 16,
    VmoRead = 17,
    VmoWrite = 18,
    VmoSupply = 19,
    VmoSnapshot = 20,
    PollCreate = 21,
    PollWait = 22,
    PollWaitUntil = 23,
    PollNotify = 24,
    NetCreate = 25,
    NetBind = 26,
    NetUnbind = 27,
    NetRecv = 28,
    NetSend = 29,
    NetSubscribe = 30,
    MonoTimeRead = 31,
    WallTimeRead = 32,
    RandomRead = 33,
    // Note: Do not forget to update Syscall::END when adding a new syscall.
}

impl Syscall {
    pub const BASE: usize = usize::MAX - 0x1000;
    const END: usize = Self::BASE + Self::RandomRead as usize;

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
