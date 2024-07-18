#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Irq(usize);

impl Irq {
    pub fn from_raw(value: usize) -> Irq {
        Irq(value)
    }
}
