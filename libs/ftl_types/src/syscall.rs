#[repr(usize)]
pub enum Syscall {
    HandleClose = 0,
    HandleDuplicate = 1,
    HSpaceCreate = 2,
    HSpaceMove = 3,
    ThreadCreate = 4,
    ThreadStart = 5,
    ThreadWriteRegs = 6,
    ThreadCopyRegs = 7,
    ThreadSubscribe = 8,
    ThreadExit = 9,
    ThreadDestroy = 10,
    QuotaCreate = 11,
    QuotaLimit = 12,
    VmSpaceCreate = 13,
    VmSpaceClone = 14,
    VmSpaceMap = 15,
    VmSpaceUnmap = 16,
    VmSpacePermit = 17,
    VmoCreateZeroed = 18,
    VmoCreateUser = 19,
    VmoSnapshot = 20,
    VmoRead = 21,
    VmoWrite = 22,
    VmoSupply = 23,
    PollCreate = 24,
    PollWait = 25,
    PollWaitUntil = 26,
    PollNotify = 27,
    NicCreateVirtual = 28,
    NicBind = 29,
    NicUnbind = 30,
    NicRecv = 31,
    NicSend = 32,
    NicSubscribe = 33,
    ConsoleRead = 34,
    ConsoleWrite = 35,
    ConsoleSubscribe = 36,
    MonoTimeRead = 37,
    WallTimeRead = 38,
    RandomRead = 39,
    ThreadYield = 40,
    // Note: Do not forget to update Syscall::END when adding a new syscall.
}

impl Syscall {
    pub const BASE: usize = usize::MAX - 0x1000;
    const END: usize = Self::BASE + Self::ThreadYield as usize;

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
