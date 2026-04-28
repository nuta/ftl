use crate::vmspace::UserCopyable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PciEntry {
    pub bus: u8,
    pub slot: u8,
    pub subsystem_vendor_id: u16,
    pub subsystem_id: u16,
}

// SAFETY: The `PciEntry` does not have padding.
unsafe impl UserCopyable for PciEntry {}
