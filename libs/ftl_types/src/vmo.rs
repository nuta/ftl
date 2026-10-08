#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyMode {
    // Copy pages.
    Copy = 0,
    // Note: Do not forget to update SupplyMode::END when adding a new mode.
}

impl SupplyMode {
    const END: usize = Self::Copy as usize;

    pub const fn from_usize(value: usize) -> Option<Self> {
        if value <= Self::END {
            // SAFETY: The value is in the enum range.
            Some(unsafe { core::mem::transmute::<usize, Self>(value) })
        } else {
            None
        }
    }
}
