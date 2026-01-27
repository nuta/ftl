use ftl_types::error::ErrorCode;
pub use ftl_types::pci::PciEntry;
use ftl_types::syscall::SYS_PCI_LOOKUP;

use crate::syscall::syscall4;

pub fn sys_pci_lookup(
    entries: *mut PciEntry,
    entries_len: usize,
    vendor: u16,
    device: u16,
) -> Result<usize, ErrorCode> {
    syscall4(
        SYS_PCI_LOOKUP,
        entries as usize,
        entries_len,
        vendor as usize,
        device as usize,
    );

    todo!()
}
