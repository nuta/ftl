use crate::error::ErrorCode;
use crate::vmspace::PageAttrs;

#[repr(usize)]
pub enum ExitReason {
    Success = 0,
    Panic = 1,
    Errored = 2,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegsKind {
    FsBase = 1,
    FpAndVector = 2,
}

impl RegsKind {
    pub const fn from_usize(value: usize) -> Option<Self> {
        match value {
            value if value == Self::FsBase as usize => Some(Self::FsBase),
            value if value == Self::FpAndVector as usize => Some(Self::FpAndVector),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Regs {
    pub fs_base: usize,
}

pub struct SyscallRegs {
    pub n: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
}

#[repr(C)]
pub struct SyscallFrame {
    pub cookie: usize,
    /// A padding to keep the frame 16-byte aligned.
    pub reserved: usize,
    pub rsp: usize,
    pub rip: usize,
}

/// The user fault type.
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    DivideError = 0,
    Debug = 1,
    Breakpoint = 3,
    InvalidOpcode = 6,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    FloatingPointError = 16,
    AlignmentCheck = 17,
    SimdFloatingPoint = 19,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FaultFrame {
    pub rflags: usize,
    pub rip: usize,
    pub cookie: usize,
    pub fault: Fault,
    pub addr: usize,
    pub info: usize,
}

/// The details of a page fault, stored in [`FaultFrame::info`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageFaultInfo(usize);

impl PageFaultInfo {
    pub const fn new(access: PageAttrs) -> Self {
        Self(access.as_raw())
    }

    pub const fn with_reason(self, reason: ErrorCode) -> Self {
        Self((self.0 & 0xff) | ((reason.as_usize() & 0xff) << 8))
    }

    pub const fn from_raw(info: usize) -> Self {
        Self(info)
    }

    pub const fn into_raw(self) -> usize {
        self.0
    }

    /// The access type that caused the page fault.
    pub const fn access(self) -> PageAttrs {
        PageAttrs::from_raw(self.0 & 0xff)
    }

    pub const fn reason(self) -> ErrorCode {
        ErrorCode::from_usize((self.0 >> 8) & 0xff)
    }
}
