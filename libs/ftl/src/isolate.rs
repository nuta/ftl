use ftl_types::handle::HandleId;

use crate::handle::OwnedHandle;

pub struct Isolate {
    handle: OwnedHandle,
}

impl Isolate {
    pub const unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub(crate) const fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
