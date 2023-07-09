use core::mem::size_of;

use essentials::static_assert;

use crate::{arch::PAGE_SIZE, process::Process, thread::Thread};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectKind {
    Unused,
    Reserved,
    Process,
    Thread,
    Channel,
    PageTable,
    SubPageTableL1,
    DataPage,
}

pub trait KernelObject {
    fn kind(&self) -> ObjectKind;
}

pub const fn object_size_for(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::Unused => 0,
        ObjectKind::Reserved => 0,
        ObjectKind::Process => PAGE_SIZE,
        ObjectKind::Thread => PAGE_SIZE,
        ObjectKind::Channel => PAGE_SIZE,
        ObjectKind::PageTable => PAGE_SIZE,
        ObjectKind::SubPageTableL1 => PAGE_SIZE,
        ObjectKind::DataPage => PAGE_SIZE,
    }
}

static_assert!(size_of::<Process>() <= object_size_for(ObjectKind::Process));
static_assert!(size_of::<Thread>() <= object_size_for(ObjectKind::Thread));
// static_assert!(size_of::<Channel>() <= object_size_for(ObjectKind::Channel));
// static_assert!(size_of::<PageTable>() <= object_size_for(ObjectKind::PageTableL0));
// static_assert!(size_of::<PageTableL1>() <= object_size_for(ObjectKind::PageTableL1));
// static_assert!(size_of::<DataPage>() <= object_size_for(ObjectKind::DataPage));
