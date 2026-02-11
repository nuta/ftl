use ftl_types::error::ErrorCode;
pub use ftl_types::pci::PciEntry;
use ftl_types::syscall::SYS_PCI_GET_BAR;
use ftl_types::syscall::SYS_PCI_GET_INTERRUPT_LINE;
use ftl_types::syscall::SYS_PCI_GET_SUBSYSTEM_ID;
use ftl_types::syscall::SYS_PCI_LOOKUP;
use ftl_types::syscall::SYS_PCI_SET_BUSMASTER;

use crate::syscall::syscall2;
use crate::syscall::syscall3;
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
    )
}

pub fn sys_pci_set_busmaster(bus: u8, slot: u8, enable: bool) -> Result<(), ErrorCode> {
    syscall3(
        SYS_PCI_SET_BUSMASTER,
        bus as usize,
        slot as usize,
        if enable { 1 } else { 0 },
    )?;

    Ok(())
}

pub fn sys_pci_get_bar(bus: u8, slot: u8, bar: u8) -> Result<u32, ErrorCode> {
    let bar = syscall3(SYS_PCI_GET_BAR, bus as usize, slot as usize, bar as usize)?;
    Ok(bar as u32)
}

pub fn sys_pci_get_subsystem_id(bus: u8, slot: u8) -> Result<u16, ErrorCode> {
    let id = syscall2(SYS_PCI_GET_SUBSYSTEM_ID, bus as usize, slot as usize)?;
    Ok(id as u16)
}

pub fn sys_pci_get_interrupt_line(bus: u8, slot: u8) -> Result<u8, ErrorCode> {
    let irq = syscall2(SYS_PCI_GET_INTERRUPT_LINE, bus as usize, slot as usize)?;
    Ok(irq as u8)
}
