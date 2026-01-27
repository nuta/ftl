#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PciEntry {
    pub bus: u8,
    pub device: u8,
}
