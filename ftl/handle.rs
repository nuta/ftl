#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u32);

impl Handle {
    pub fn from_raw(value: u32) -> Handle {
        Handle(value)
    }
}
