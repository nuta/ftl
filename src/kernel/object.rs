#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectKind {
    Unused,
    Reserved,
    Process,
    Thread,
    Channel,
    PageTableL0,
    PageTableL1,
    DataPage,
}

pub trait KernelObject {
    fn kind(&self) -> ObjectKind;
}
