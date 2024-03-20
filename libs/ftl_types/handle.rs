use core::num::NonZeroIsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(NonZeroIsize);

impl HandleId {
    pub const fn from_nonzero(id: NonZeroIsize) -> HandleId {
        Self(id)
    }
}
