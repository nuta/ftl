pub const UNLIMITED: usize = usize::MAX;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Memory = 0,
    CpuTime = 1,
}

impl Resource {
    pub const COUNT: usize = 2;

    pub const fn from_usize(value: usize) -> Option<Self> {
        if value >= Self::COUNT {
            return None;
        }

        // SAFETY: The value is guaranteed to be within the range of the enum.
        Some(unsafe { core::mem::transmute(value) })
    }
}
