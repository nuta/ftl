#[repr(usize)]
pub enum Syscall {
    HSpaceClose = 0,
    ConsoleRead = 1,
    ConsoleWrite = 2,
    ConsoleSubscribe = 3,
    ThreadCreate = 4,
    ThreadStart = 5,
    ThreadWriteRegs = 6,
    ThreadCopyRegs = 7,
    ThreadSubscribe = 8,
    ThreadExit = 9,
    VmSpaceClone = 10,
    VmSpaceMap = 11,
    VmSpaceUnmap = 12,
    VmSpacePermit = 13,
    VmoCreateZeroed = 14,
    VmoCreateUser = 15,
    VmoRead = 16,
    VmoWrite = 17,
    VmoSupply = 18,
    VmoSnapshot = 19,
    PollCreate = 20,
    PollWait = 21,
    PollWaitUntil = 22,
    PollNotify = 23,
    NetCreate = 24,
    NetBind = 25,
    NetUnbind = 26,
    NetRecv = 27,
    NetSend = 28,
    NetSubscribe = 29,
    MonoTimeRead = 30,
    WallTimeRead = 31,
    RandomRead = 32,
    HSpaceCreate = 33,
    QuotaLimit = 34,
    VmSpaceCreate = 35,
    HSpaceMove = 36,
    QuotaCreate = 37,
    // Note: Do not forget to update Syscall::END when adding a new syscall.
}

impl Syscall {
    pub const BASE: usize = usize::MAX - 0x1000;
    const END: usize = Self::BASE + Self::QuotaCreate as usize;

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
