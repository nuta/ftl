/// A handle ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct HandleId(usize);

impl HandleId {
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
