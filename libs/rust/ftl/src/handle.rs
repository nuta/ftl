use core::fmt;

use ftl_types::handle::HandleId;

pub struct OwnedHandle(HandleId);

impl OwnedHandle {
    pub fn from_raw(id: HandleId) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> HandleId {
        self.0
    }

    pub(crate) const fn as_usize(&self) -> usize {
        self.0.as_usize()
    }
}

impl fmt::Debug for OwnedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OwnedHandle").field(&self.0).finish()
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        println!("dropping handle {:?}", self.0);
        // TODO:
    }
}

pub trait Handleable {
    fn handle(&self) -> &OwnedHandle;
}
