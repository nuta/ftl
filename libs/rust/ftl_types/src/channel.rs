#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const fn len(&self) -> usize {
        self.0 as usize
    }
}
