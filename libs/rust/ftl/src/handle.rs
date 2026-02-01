use core::fmt;

// TODO: Make this private
pub use ftl_types::handle::HandleId;

pub struct OwnedHandle(HandleId);

impl OwnedHandle {
    // TODO: Make this private
    pub const fn from_raw(id: HandleId) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> HandleId {
        self.0
    }
}

impl fmt::Debug for OwnedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedHandle")
            .field(&self.0.as_usize())
            .finish()
    }
}

pub trait Handleable {
    fn handle(&self) -> &OwnedHandle;
}
